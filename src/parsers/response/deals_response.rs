//! Response parser for `GetFlightDealsStreaming`.
//!
//! Same streaming `wrb.fr` envelope as explore. The deals live at JSON path
//! `arr[3][9]` in the chunk that carries them. Each deal entry (verified from
//! live captures):
//!
//! ```text
//! [1]  outbound date  [Y, M, D]
//! [2]  return date    [Y, M, D]
//! [3]  [[null, price], "booking_token"]
//! [4]  [[null, typical_price]]
//! [5]  discount_pct
//! [6]  [null, null, "/travel/flights?tfs=…"]   (booking deep link path)
//! [7]  duration_minutes
//! [8]  stops
//! [10] airline_code      ("*" for mixed)
//! [11] airline_name
//! [13] [city, country, image_url, [highlights…], null, description]
//! [17] origin IATA
//! [18] destination IATA
//! [22] ["/m/MID", 4]     destination place MID
//! ```

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;

use crate::parsers::common::{decode_inner_object, decode_outer_object, GetOuterErrorMessages};
use crate::requests::config::deals::DealResult;

const GOOGLE_ORIGIN: &str = "https://www.google.com";

// Each response line: [["wrb.fr", null, payload_str, ...], ...]; payload at [2].
#[derive(Debug)]
pub(crate) struct DealsRawChunk {
    pub payload: Option<String>,
}

impl<'de> serde::Deserialize<'de> for DealsRawChunk {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let outer = Vec::<Value>::deserialize(d)?;
        let wrb_entry: Vec<Value> = crate::parsers::common::get_idx(&outer, 0)
            .ok_or_else(|| serde::de::Error::custom("missing wrb.fr entry at [0]"))?;
        let payload: Option<String> = crate::parsers::common::get_idx(&wrb_entry, 2);
        Ok(DealsRawChunk { payload })
    }
}

impl GetOuterErrorMessages for DealsRawChunk {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        None
    }
}

/// Parse a raw `GetFlightDealsStreaming` HTTP response body.
///
/// Returns an empty `Vec` on any structural parse failure (e.g. HTML / consent
/// bodies) rather than propagating, so callers always get a usable result.
pub fn parse_deals_response(raw: &str) -> Result<Vec<DealResult>> {
    let chunks: Vec<DealsRawChunk> = match decode_outer_object(raw) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "deals: failed to decode outer object");
            return Ok(vec![]);
        }
    };

    let mut deals: Vec<DealResult> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for chunk in &chunks {
        let payload = match chunk.payload.as_deref() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let arr: Vec<Value> = match decode_inner_object(payload) {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(error = %e, "deals: failed to decode inner payload");
                continue;
            }
        };

        // Deals list at arr[3][9].
        let list = match arr
            .get(3)
            .and_then(|v| v.as_array())
            .and_then(|c| c.get(9))
            .and_then(|v| v.as_array())
        {
            Some(l) => l,
            None => continue,
        };

        for entry in list {
            let e = match entry.as_array() {
                Some(a) => a,
                None => continue,
            };
            // Guard: a real deal entry has a [Y,M,D] date at [1].
            let out_date = match ymd(e.get(1)) {
                Some(d) => d,
                None => continue,
            };
            let deal = parse_deal_entry(e, out_date);
            let key = (deal.destination_iata.clone(), date_key(deal.outbound_date));
            if seen.insert(key) {
                deals.push(deal);
            }
        }
    }

    Ok(deals)
}

fn parse_deal_entry(e: &[Value], out_date: NaiveDate) -> DealResult {
    // [3] = [[null, price], "booking_token"]
    let price = e
        .get(3)
        .and_then(|v| v.as_array())
        .and_then(|p| p.first())
        .and_then(|v| v.as_array())
        .and_then(|inner| inner.get(1))
        .and_then(|v| v.as_i64())
        .map(|n| n as i32);
    let booking_token = e
        .get(3)
        .and_then(|v| v.as_array())
        .and_then(|p| p.get(1))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // [4] = [[null, typical_price]]
    let typical_price = e
        .get(4)
        .and_then(|v| v.as_array())
        .and_then(|p| p.first())
        .and_then(|v| v.as_array())
        .and_then(|inner| inner.get(1))
        .and_then(|v| v.as_i64())
        .map(|n| n as i32);

    let discount_pct = e.get(5).and_then(|v| v.as_i64()).map(|n| n as i32);

    // [6] = [null, null, "/travel/flights?tfs=…"]
    let booking_url = e
        .get(6)
        .and_then(|v| v.as_array())
        .and_then(|b| b.get(2))
        .and_then(|v| v.as_str())
        .map(|path| {
            if path.starts_with("http") {
                path.to_string()
            } else {
                format!("{GOOGLE_ORIGIN}{path}")
            }
        });

    let duration_minutes = e.get(7).and_then(|v| v.as_i64()).map(|n| n.max(0) as u32);
    let stops = e
        .get(8)
        .and_then(|v| v.as_i64())
        .map(|n| n.clamp(0, 255) as u8);

    let airline_code = e.get(10).and_then(|v| v.as_str()).map(str::to_string);
    let airline_name = e.get(11).and_then(|v| v.as_str()).map(str::to_string);

    // [13] = [city, country, image_url, [highlights], null, description]
    let dest = e.get(13).and_then(|v| v.as_array());
    let destination_city = dest
        .and_then(|d| d.first())
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let destination_country = dest
        .and_then(|d| d.get(1))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let image_url = dest
        .and_then(|d| d.get(2))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let highlights = dest
        .and_then(|d| d.get(3))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let description = dest
        .and_then(|d| d.get(5))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let origin_iata = e
        .get(17)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let destination_iata = e
        .get(18)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let destination_mid = e
        .get(22)
        .and_then(|v| v.as_array())
        .and_then(|m| m.first())
        .and_then(|v| v.as_str())
        .map(str::to_string);

    DealResult {
        origin_iata,
        destination_iata,
        destination_city,
        destination_country,
        destination_mid,
        outbound_date: Some(out_date),
        return_date: ymd(e.get(2)),
        price,
        typical_price,
        discount_pct,
        duration_minutes,
        stops,
        airline_code,
        airline_name,
        image_url,
        highlights,
        description,
        booking_url,
        booking_token,
    }
}

/// Parse a `[Y, M, D]` integer triple into a `NaiveDate`.
fn ymd(v: Option<&Value>) -> Option<NaiveDate> {
    let a = v?.as_array()?;
    let y = a.first()?.as_i64()? as i32;
    let m = a.get(1)?.as_i64()? as u32;
    let d = a.get(2)?.as_i64()? as u32;
    NaiveDate::from_ymd_opt(y, m, d)
}

fn date_key(d: Option<NaiveDate>) -> String {
    d.map(|x| x.to_string()).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_non_deals_body_returns_empty() {
        let raw = ")]}'\n\n[[\"noop\"]]\n";
        assert!(parse_deals_response(raw).unwrap().is_empty());
    }

    #[test]
    fn ymd_parses_triple() {
        let v = serde_json::json!([2026, 9, 24]);
        assert_eq!(ymd(Some(&v)), NaiveDate::from_ymd_opt(2026, 9, 24));
        assert_eq!(ymd(Some(&serde_json::json!([]))), None);
        assert_eq!(ymd(None), None);
    }

    #[test]
    fn parse_deal_entry_extracts_fields() {
        // Minimal entry mirroring the wire layout (indices that matter populated).
        let mut e = vec![Value::Null; 24];
        e[1] = serde_json::json!([2026, 9, 24]);
        e[2] = serde_json::json!([2026, 9, 28]);
        e[3] = serde_json::json!([[Value::Null, 71], "tok_b64"]);
        e[4] = serde_json::json!([[Value::Null, 221]]);
        e[5] = serde_json::json!(68);
        e[6] = serde_json::json!([Value::Null, Value::Null, "/travel/flights?tfs=ABC"]);
        e[7] = serde_json::json!(175);
        e[8] = serde_json::json!(0);
        e[10] = serde_json::json!("U2");
        e[11] = serde_json::json!("easyJet");
        e[13] = serde_json::json!([
            "Lisbon",
            "Portugal",
            "https://img",
            ["Alfama & fado"],
            Value::Null,
            "Hilly capital."
        ]);
        e[17] = serde_json::json!("LUX");
        e[18] = serde_json::json!("LIS");
        e[22] = serde_json::json!(["/m/04llb", 4]);

        let d = parse_deal_entry(&e, NaiveDate::from_ymd_opt(2026, 9, 24).unwrap());
        assert_eq!(d.origin_iata, "LUX");
        assert_eq!(d.destination_iata, "LIS");
        assert_eq!(d.destination_city, "Lisbon");
        assert_eq!(d.destination_country, "Portugal");
        assert_eq!(d.price, Some(71));
        assert_eq!(d.typical_price, Some(221));
        assert_eq!(d.discount_pct, Some(68));
        assert_eq!(d.duration_minutes, Some(175));
        assert_eq!(d.stops, Some(0));
        assert_eq!(d.airline_code.as_deref(), Some("U2"));
        assert_eq!(d.airline_name.as_deref(), Some("easyJet"));
        assert_eq!(d.destination_mid.as_deref(), Some("/m/04llb"));
        assert_eq!(d.highlights, vec!["Alfama & fado".to_string()]);
        assert_eq!(
            d.booking_url.as_deref(),
            Some("https://www.google.com/travel/flights?tfs=ABC")
        );
        assert_eq!(d.return_date, NaiveDate::from_ymd_opt(2026, 9, 28));
    }

    #[test]
    fn full_envelope_extracts_deal() {
        // Build a wrb.fr envelope with arr[3][9] = [deal].
        let mut e = vec![Value::Null; 19];
        e[1] = serde_json::json!([2026, 9, 24]);
        e[2] = serde_json::json!([2026, 9, 28]);
        e[3] = serde_json::json!([[Value::Null, 71], "tok"]);
        e[17] = serde_json::json!("LUX");
        e[18] = serde_json::json!("LIS");
        let mut arr3 = vec![Value::Null; 10];
        arr3[9] = serde_json::json!([Value::Array(e)]);
        let inner = serde_json::json!([Value::Null, Value::Null, Value::Null, Value::Array(arr3)]);
        let inner_str = serde_json::to_string(&inner).unwrap();
        let line = serde_json::to_string(&serde_json::json!([["wrb.fr", Value::Null, inner_str]]))
            .unwrap();
        let raw = format!(")]}}'\n\n{}\n{}\n", line.len(), line);

        let deals = parse_deals_response(&raw).unwrap();
        assert_eq!(deals.len(), 1);
        assert_eq!(deals[0].destination_iata, "LIS");
        assert_eq!(deals[0].price, Some(71));
    }
}
