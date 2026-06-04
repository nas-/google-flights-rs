//! MCP (Model Context Protocol) server over stdio.
//!
//! Speaks JSON-RPC 2.0 with newline-delimited messages on stdin/stdout — the
//! MCP stdio transport — so it works with any MCP client (e.g. Claude Desktop).
//! Each tool is a thin adapter: it parses JSON arguments, builds the existing
//! `Config`/`ExploreConfig`, calls the corresponding `ApiClient` method, and
//! returns the serialized result. No new business logic lives here.
//!
//! Supported tools: `search`, `price_graph`, `cheapest_dates`, `explore`.

use anyhow::Result;
use chrono::{Months, NaiveDate};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdout};

use gflights::parsers::common::{Location, PlaceType, StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Config, Currency, ExploreConfig, ExploreDate};

/// MCP protocol revision this server implements.
const PROTOCOL_VERSION: &str = "2025-06-18";
const SERVER_NAME: &str = "gflights";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Server loop
// ---------------------------------------------------------------------------

pub async fn run_mcp(client: &ApiClient) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                send_error(&mut out, Value::Null, -32700, &format!("parse error: {e}")).await?;
                continue;
            }
        };

        let id = msg.get("id").cloned(); // absent => notification (no response)
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                send_result(&mut out, id, initialize_result()).await?;
            }
            "ping" => {
                send_result(&mut out, id, json!({})).await?;
            }
            "tools/list" => {
                send_result(&mut out, id, json!({ "tools": tool_catalog() })).await?;
            }
            "tools/call" => {
                let result = handle_tool_call(&params, client).await;
                send_result(&mut out, id, tool_result(result)).await?;
            }
            // Notifications (initialized, cancelled, …) require no response.
            m if m.starts_with("notifications/") => {}
            _ => {
                // Only requests (those carrying an id) get an error reply.
                if let Some(id) = id {
                    send_error(&mut out, id, -32601, &format!("method not found: {method}"))
                        .await?;
                }
            }
        }
    }
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

/// Wrap a tool outcome into an MCP `tools/call` result object.
fn tool_result(outcome: std::result::Result<String, String>) -> Value {
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(msg) => json!({ "content": [{ "type": "text", "text": msg }], "isError": true }),
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC framing helpers
// ---------------------------------------------------------------------------

async fn write_line(out: &mut Stdout, v: &Value) -> Result<()> {
    let mut s = serde_json::to_string(v)?;
    s.push('\n');
    out.write_all(s.as_bytes()).await?;
    out.flush().await?;
    Ok(())
}

async fn send_result(out: &mut Stdout, id: Option<Value>, result: Value) -> Result<()> {
    // A response is only meaningful for a request (id present).
    let Some(id) = id else { return Ok(()) };
    write_line(
        out,
        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    )
    .await
}

async fn send_error(out: &mut Stdout, id: Value, code: i64, message: &str) -> Result<()> {
    write_line(
        out,
        &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    )
    .await
}

// ---------------------------------------------------------------------------
// Tool catalog (name, description, JSON-Schema for arguments)
// ---------------------------------------------------------------------------

fn tool_catalog() -> Vec<Value> {
    let route_props = json!({
        "from": { "type": "string", "description": "Departure IATA code or city name" },
        "to": { "type": "string", "description": "Destination IATA code or city name" },
        "date": { "type": "string", "description": "Departure date YYYY-MM-DD" },
        "return_date": { "type": "string", "description": "Return date YYYY-MM-DD (omit for one-way)" },
        "adults": { "type": "integer", "minimum": 1, "default": 1 },
        "class": { "type": "string", "enum": ["economy", "premium-economy", "business", "first"] },
        "stops": { "type": "string", "enum": ["all", "nonstop", "one-stop"] },
        "currency": { "type": "string", "description": "e.g. euro, us-dollar" },
        "lang": { "type": "string", "description": "BCP-47 language subtag, default en" },
        "country": { "type": "string", "description": "ISO 3166-1 alpha-2, default GB" }
    });

    vec![
        json!({
            "name": "search",
            "description": "Search flights for a route and date (one-way or round-trip). Returns itineraries with price, stops, duration, and legs.",
            "inputSchema": { "type": "object", "properties": route_props, "required": ["from", "to", "date"] }
        }),
        json!({
            "name": "price_graph",
            "description": "Cheapest fare per departure day over N months for a route. Returns [{date, price}].",
            "inputSchema": {
                "type": "object",
                "properties": json!({
                    "from": { "type": "string" }, "to": { "type": "string" },
                    "date": { "type": "string", "description": "Start date YYYY-MM-DD" },
                    "months": { "type": "integer", "minimum": 1, "default": 3 },
                    "adults": { "type": "integer", "minimum": 1, "default": 1 },
                    "currency": { "type": "string" }, "lang": { "type": "string" }, "country": { "type": "string" }
                }),
                "required": ["from", "to", "date"]
            }
        }),
        json!({
            "name": "cheapest_dates",
            "description": "Cheapest departure dates over N months. Set trip_days for round trips of that length; omit for one-way.",
            "inputSchema": {
                "type": "object",
                "properties": json!({
                    "from": { "type": "string" }, "to": { "type": "string" },
                    "date": { "type": "string", "description": "Earliest departure date YYYY-MM-DD" },
                    "months": { "type": "integer", "minimum": 1, "default": 3 },
                    "trip_days": { "type": "integer", "description": "Round-trip length in nights; omit for one-way" },
                    "adults": { "type": "integer", "minimum": 1, "default": 1 },
                    "currency": { "type": "string" }, "lang": { "type": "string" }, "country": { "type": "string" }
                }),
                "required": ["from", "to", "date"]
            }
        }),
        json!({
            "name": "explore",
            "description": "Explore cheap destinations from an origin airport. Optional destination airport, travel month, and budget.",
            "inputSchema": {
                "type": "object",
                "properties": json!({
                    "from": { "type": "string", "description": "Origin IATA code" },
                    "to": { "type": "string", "description": "Optional destination IATA code" },
                    "month": { "type": "integer", "minimum": 1, "maximum": 12 },
                    "budget": { "type": "integer", "description": "Max price in the chosen currency" },
                    "adults": { "type": "integer", "minimum": 1, "default": 1 },
                    "currency": { "type": "string" }, "lang": { "type": "string" }, "country": { "type": "string" }
                }),
                "required": ["from"]
            }
        }),
    ]
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

async fn handle_tool_call(
    params: &Value,
    client: &ApiClient,
) -> std::result::Result<String, String> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| "tools/call missing 'name'".to_string())?;
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    match name {
        "search" => tool_search(&args, client).await,
        "price_graph" => tool_price_graph(&args, client).await,
        "cheapest_dates" => tool_cheapest_dates(&args, client).await,
        "explore" => tool_explore(&args, client).await,
        other => Err(format!("unknown tool: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

fn req_str(args: &Value, key: &str) -> std::result::Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("missing or non-string argument: {key}"))
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

fn opt_u32(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
}

fn parse_date(s: &str) -> std::result::Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("invalid date {s:?}: {e}"))
}

fn parse_class(s: &str) -> std::result::Result<TravelClass, String> {
    match s.to_lowercase().as_str() {
        "economy" | "eco" => Ok(TravelClass::Economy),
        "premium-economy" | "premium_economy" => Ok(TravelClass::PremiumEconomy),
        "business" | "biz" => Ok(TravelClass::Business),
        "first" => Ok(TravelClass::First),
        _ => Err(format!("unknown class {s:?}")),
    }
}

fn parse_currency(s: &str) -> std::result::Result<Currency, String> {
    <Currency as clap::ValueEnum>::from_str(s, true)
        .map_err(|e| format!("unknown currency {s:?}: {e}"))
}

fn parse_stops(s: &str) -> std::result::Result<StopOptions, String> {
    match s.to_lowercase().as_str() {
        "all" | "any" => Ok(StopOptions::All),
        "nonstop" | "non-stop" | "direct" => Ok(StopOptions::NoStop),
        "one-stop" | "one_stop" | "onestop" => Ok(StopOptions::OneOrLess),
        _ => Err(format!("unknown stops {s:?}")),
    }
}

fn travelers_for(adults: u32) -> std::result::Result<Travelers, String> {
    Travelers::new(vec![adults as i32, 0, 0, 0]).map_err(|e| e.to_string())
}

/// Build a route `Config` from the common argument set shared by search,
/// price_graph, and cheapest_dates. `with_return` controls whether a
/// `return_date` argument is honoured.
async fn build_route_config(
    args: &Value,
    client: &ApiClient,
    with_return: bool,
) -> std::result::Result<Config, String> {
    let from = req_str(args, "from")?;
    let to = req_str(args, "to")?;
    let date = parse_date(&req_str(args, "date")?)?;
    let adults = opt_u32(args, "adults").unwrap_or(1);

    let mut b = Config::builder()
        .departure(&from, client)
        .await
        .map_err(|e| e.to_string())?
        .destination(&to, client)
        .await
        .map_err(|e| e.to_string())?
        .departing_date(date)
        .travelers(travelers_for(adults)?)
        .currency(parse_currency(
            &opt_str(args, "currency").unwrap_or_else(|| "euro".into()),
        )?)
        .language(opt_str(args, "lang").unwrap_or_else(|| "en".into()))
        .country(opt_str(args, "country").unwrap_or_else(|| "GB".into()));

    if let Some(c) = opt_str(args, "class") {
        b = b.travel_class(parse_class(&c)?);
    }
    if let Some(s) = opt_str(args, "stops") {
        b = b.stop_options(parse_stops(&s)?);
    }
    if with_return {
        if let Some(ret) = opt_str(args, "return_date") {
            b = b.return_date(parse_date(&ret)?);
        }
    }

    b.build().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn tool_search(args: &Value, client: &ApiClient) -> std::result::Result<String, String> {
    let config = build_route_config(args, client, true).await?;
    let res = client
        .request_flights(&config)
        .await
        .map_err(|e| e.to_string())?;
    let flights = res.get_all_flights();
    serde_json::to_string(&flights).map_err(|e| e.to_string())
}

async fn tool_price_graph(args: &Value, client: &ApiClient) -> std::result::Result<String, String> {
    let config = build_route_config(args, client, false).await?;
    let months = Months::new(opt_u32(args, "months").unwrap_or(3));
    let graph = client
        .request_graph(&config, months)
        .await
        .map_err(|e| e.to_string())?;
    let mut points: Vec<_> = graph
        .get_all_graphs()
        .into_iter()
        .filter_map(|g| g.maybe_get_date_price())
        .map(|(d, p)| json!({ "date": d.to_string(), "price": p }))
        .collect();
    points.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    serde_json::to_string(&points).map_err(|e| e.to_string())
}

async fn tool_cheapest_dates(
    args: &Value,
    client: &ApiClient,
) -> std::result::Result<String, String> {
    let config = build_route_config(args, client, false).await?;
    let months = Months::new(opt_u32(args, "months").unwrap_or(3));
    let trip_days = opt_u32(args, "trip_days");
    let results = client
        .cheapest_dates(&config, months, trip_days)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

async fn tool_explore(args: &Value, client: &ApiClient) -> std::result::Result<String, String> {
    let from = req_str(args, "from")?;
    let origin = Location {
        loc_identifier: from.to_uppercase(),
        loc_type: PlaceType::Airport,
        location_name: None,
    };
    let destination = opt_str(args, "to").map(|t| Location {
        loc_identifier: t.to_uppercase(),
        loc_type: PlaceType::Airport,
        location_name: None,
    });
    let trip_date = opt_u32(args, "month").map(|m| ExploreDate { month: m as u8 });
    let adults = opt_u32(args, "adults").unwrap_or(1);

    let config = ExploreConfig {
        origin: vec![origin],
        destination,
        trip_date,
        max_price: opt_u32(args, "budget").map(|b| b as i32),
        travellers: travelers_for(adults)?,
        currency: parse_currency(&opt_str(args, "currency").unwrap_or_else(|| "euro".into()))?,
        language: opt_str(args, "lang").unwrap_or_else(|| "en".into()),
        country: opt_str(args, "country").unwrap_or_else(|| "GB".into()),
        ..Default::default()
    };

    let mut results = client
        .request_explore(&config)
        .await
        .map_err(|e| e.to_string())?;
    results.retain(|r| r.price.is_some());
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_catalog_has_expected_tools() {
        let names: Vec<String> = tool_catalog()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"search".to_string()));
        assert!(names.contains(&"price_graph".to_string()));
        assert!(names.contains(&"cheapest_dates".to_string()));
        assert!(names.contains(&"explore".to_string()));
    }

    #[test]
    fn every_tool_has_name_description_and_schema() {
        for t in tool_catalog() {
            assert!(t["name"].as_str().is_some());
            assert!(t["description"].as_str().is_some());
            assert_eq!(t["inputSchema"]["type"].as_str(), Some("object"));
            assert!(t["inputSchema"]["properties"].is_object());
        }
    }

    #[test]
    fn initialize_result_advertises_tools_capability() {
        let r = initialize_result();
        assert_eq!(r["protocolVersion"].as_str(), Some(PROTOCOL_VERSION));
        assert!(r["capabilities"]["tools"].is_object());
        assert_eq!(r["serverInfo"]["name"].as_str(), Some(SERVER_NAME));
    }

    #[test]
    fn tool_result_marks_errors() {
        let ok = tool_result(Ok("[]".into()));
        assert_eq!(ok["isError"].as_bool(), Some(false));
        assert_eq!(ok["content"][0]["text"].as_str(), Some("[]"));

        let err = tool_result(Err("boom".into()));
        assert_eq!(err["isError"].as_bool(), Some(true));
        assert_eq!(err["content"][0]["text"].as_str(), Some("boom"));
    }

    #[test]
    fn parse_helpers_validate_input() {
        assert!(parse_date("2026-09-15").is_ok());
        assert!(parse_date("nope").is_err());
        assert!(parse_class("business").is_ok());
        assert!(parse_class("zzz").is_err());
        assert!(parse_stops("nonstop").is_ok());
        assert!(parse_stops("zzz").is_err());
    }

    #[test]
    fn req_str_reports_missing() {
        let v = json!({ "from": "LHR" });
        assert_eq!(req_str(&v, "from").unwrap(), "LHR");
        assert!(req_str(&v, "to").is_err());
    }
}
