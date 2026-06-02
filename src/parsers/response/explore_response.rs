//! Response parser for `GetExploreDestinations`.
//!
//! # Parsing notes (best-effort)
//!
//! The response is a streaming JSONL payload identical in outer structure to
//! other Google Flights endpoints: the first line is `)]}'`, followed by pairs
//! of (length, data) lines, where data lines are `[["wrb.fr", ...]]` JSON.
//!
//! The inner payload contains multiple numbered messages:
//!
//! - **Message type 1** (discriminant observed at the inner-array level):
//!   contains a list of destination entries.  Each entry has at least a place
//!   ID string (a `/m/…` or `/g/…` KG MID), coordinates, city/country names,
//!   nearest airport IATA code, and an opaque booking token.
//!
//! - **Message type 2**: contains flight-detail overlays keyed by destination —
//!   price, airline, stop count, duration, and an accommodation price.
//!
//! **Uncertainty**: this format has been decoded without live request capture.
//! The exact array positions of every field are inferred from the task spec and
//! the structure of analogous endpoints.  Fields that cannot be reliably located
//! are returned as `None`.  The parser never errors on a malformed response;
//! instead it returns as many results as it can extract and logs warnings via
//! `tracing`.
//!
//! If you have a live response and the results look wrong, compare the raw JSON
//! against the index comments in [`parse_destination_entry`] and
//! [`parse_flight_detail`] and adjust the `get_idx` calls accordingly.

use anyhow::Result;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::parsers::common::{decode_outer_object, get_idx, GetOuterErrorMessages};
use crate::requests::config::explore::ExploreResult;

// ---------------------------------------------------------------------------
// Raw outer-wrapper types  (mirrors the structure used by other response mods)
// ---------------------------------------------------------------------------

/// One `["wrb.fr", …]` chunk from the streaming response.
#[derive(Debug, Deserialize)]
pub(crate) struct ExploreRawChunk {
    #[serde(rename = "1")]
    pub resp: Vec<ExploreRawMessage>,
}

/// One message inside a chunk.
#[derive(Debug, Deserialize)]
pub(crate) struct ExploreRawMessage {
    /// The inner payload string (3rd element of the wrb.fr array).
    #[serde(rename = "2")]
    pub payload: Option<String>,
}

impl GetOuterErrorMessages for ExploreRawChunk {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        None // explore endpoint does not embed error strings
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse a raw `GetExploreDestinations` HTTP response body.
///
/// Returns an empty `Vec` on any structural parse failure rather than
/// propagating an error, so callers always get a usable (possibly empty) result.
pub fn parse_explore_response(raw: &str) -> Result<Vec<ExploreResult>> {
    // Step 1: decode the outer wrb.fr envelope (same helper used everywhere).
    let chunks: Vec<ExploreRawChunk> = match decode_outer_object(raw) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "explore: failed to decode outer object");
            return Ok(vec![]);
        }
    };

    // Step 2: collect all inner payload strings.
    let payloads: Vec<&str> = chunks
        .iter()
        .flat_map(|c| &c.resp)
        .filter_map(|m| m.payload.as_deref())
        .collect();

    if payloads.is_empty() {
        tracing::debug!("explore: no inner payloads found");
        return Ok(vec![]);
    }

    // Step 3: parse destinations (msg type 1) and flight details (msg type 2).
    let mut destinations: Vec<ExploreResult> = Vec::new();
    let mut flight_details: HashMap<String, FlightDetail> = HashMap::new();

    for payload in &payloads {
        if let Ok(arr) = crate::parsers::common::decode_inner_object::<Vec<Value>>(payload) {
            // Heuristic: if arr[0] is an array containing arrays of 4+ elements
            // with a string-looking MID as the first element → msg type 1 (destinations).
            // Otherwise treat as msg type 2 (flight details).
            if looks_like_destination_list(&arr) {
                for entry in arr.into_iter() {
                    if let Ok(dest) = parse_destination_entry(entry) {
                        destinations.push(dest);
                    }
                }
            } else {
                // Try to extract flight detail overlays.
                if let Ok(details) = parse_flight_details_message(&arr) {
                    for (place_id, detail) in details {
                        flight_details.insert(place_id, detail);
                    }
                }
            }
        }
    }

    // Step 4: merge flight details into destinations.
    for dest in &mut destinations {
        if let Some(detail) = flight_details.get(&dest.place_id) {
            dest.price = detail.price;
            dest.airline = detail.airline.clone();
            dest.stops = detail.stops;
            dest.flight_duration_minutes = detail.flight_duration_minutes;
            dest.accommodation_price = detail.accommodation_price;
        }
    }

    Ok(destinations)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Shallow check: does this array look like a list of destination entries?
///
/// A destination entry should be an array whose first element is a string that
/// looks like a Google KG MID (`/m/…`, `/g/…`) or an IATA code.
fn looks_like_destination_list(arr: &[Value]) -> bool {
    arr.first()
        .and_then(|v| v.as_array())
        .and_then(|inner| inner.first())
        .map(|v| v.is_string())
        .unwrap_or(false)
}

/// Intermediate flight-detail overlay for a single destination.
#[derive(Debug, Default)]
struct FlightDetail {
    price: Option<i32>,
    airline: Option<String>,
    stops: Option<u8>,
    flight_duration_minutes: Option<u32>,
    accommodation_price: Option<i32>,
}

/// Parse one destination entry from message type 1.
///
/// Observed/inferred wire structure (positions may shift):
/// ```text
/// arr[0]  = place_id       (String, e.g. "/m/0vzm")
/// arr[1]  = name           (String)
/// arr[2]  = country        (String)
/// arr[3]  = coords         ([lat, lng])
/// arr[4]  = image_url      (String or null)
/// arr[5]  = nearest_airport (String, IATA)
/// arr[6]  = date_from      (String "YYYY-MM-DD" or null)
/// arr[7]  = date_to        (String "YYYY-MM-DD" or null)
/// arr[8]  = booking_token  (String)
/// ```
///
/// Many of these positions are uncertain — fields that cannot be extracted
/// are returned as `None` / empty string.
fn parse_destination_entry(v: Value) -> Result<ExploreResult> {
    let arr = match v {
        Value::Array(a) => a,
        _ => anyhow::bail!("destination entry is not an array"),
    };

    let place_id: String = get_idx(&arr, 0).unwrap_or_default();
    if place_id.is_empty() {
        anyhow::bail!("destination entry has empty place_id");
    }

    let name: String = get_idx(&arr, 1).unwrap_or_default();
    let country: String = get_idx(&arr, 2).unwrap_or_default();

    // Coordinates: expect [lat, lng] at index 3.
    let coords_arr: Vec<f64> = get_idx(&arr, 3).unwrap_or_default();
    let coords = (
        coords_arr.first().copied().unwrap_or(0.0),
        coords_arr.get(1).copied().unwrap_or(0.0),
    );

    let image_url: Option<String> = get_idx(&arr, 4);
    let nearest_airport: String = get_idx(&arr, 5).unwrap_or_default();

    let date_from: Option<NaiveDate> = get_idx::<String>(&arr, 6)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let date_to: Option<NaiveDate> = get_idx::<String>(&arr, 7)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let booking_token: String = get_idx(&arr, 8).unwrap_or_default();

    Ok(ExploreResult {
        place_id,
        name,
        country,
        coords,
        image_url,
        nearest_airport,
        date_from,
        date_to,
        price: None,
        airline: None,
        stops: None,
        flight_duration_minutes: None,
        accommodation_price: None,
        booking_token,
    })
}

/// Parse a message type 2 payload (flight-detail overlays).
///
/// Inferred structure:
/// ```text
/// outer array of entries, each:
///   entry[0] = place_id   (String)
///   entry[1] = detail_arr (Array)
///     detail_arr[0] = price                    (i32)
///     detail_arr[1] = airline_code             (String)
///     detail_arr[2] = stops                    (i32)
///     detail_arr[3] = flight_duration_minutes  (i32)
///     detail_arr[4] = accommodation_price      (i32 or null)
/// ```
fn parse_flight_details_message(arr: &[Value]) -> Result<HashMap<String, FlightDetail>> {
    let mut map = HashMap::new();

    for entry in arr {
        let entry_arr = match entry.as_array() {
            Some(a) => a,
            None => continue,
        };

        let place_id: String = match get_idx(entry_arr, 0) {
            Some(s) => s,
            None => continue,
        };

        let detail_arr: Vec<Value> = get_idx(entry_arr, 1).unwrap_or_default();

        let price: Option<i32> = get_idx(&detail_arr, 0);
        let airline: Option<String> = get_idx(&detail_arr, 1);
        let stops: Option<u8> = get_idx::<i64>(&detail_arr, 2).map(|n| n.min(255) as u8);
        let flight_duration_minutes: Option<u32> =
            get_idx::<i64>(&detail_arr, 3).map(|n| n.max(0) as u32);
        let accommodation_price: Option<i32> = get_idx(&detail_arr, 4);

        map.insert(
            place_id,
            FlightDetail {
                price,
                airline,
                stops,
                flight_duration_minutes,
                accommodation_price,
            },
        );
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_response_returns_empty_vec() {
        // A well-formed but empty outer envelope.
        let raw = ")]}'
\n
[[\"noop\"]]
\n
";
        let results = parse_explore_response(raw).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_destination_entry_happy_path() {
        let entry = serde_json::json!([
            "/m/0vzm",
            "Vienna",
            "Austria",
            [48.2082, 16.3738],
            "https://example.com/img.jpg",
            "VIE",
            "2026-08-01",
            "2026-08-08",
            "tok_abc"
        ]);
        let result = parse_destination_entry(entry).unwrap();
        assert_eq!(result.place_id, "/m/0vzm");
        assert_eq!(result.name, "Vienna");
        assert_eq!(result.country, "Austria");
        assert!((result.coords.0 - 48.2082).abs() < 1e-3);
        assert_eq!(result.nearest_airport, "VIE");
        assert_eq!(result.booking_token, "tok_abc");
        assert!(result.date_from.is_some());
        assert!(result.date_to.is_some());
    }

    #[test]
    fn parse_destination_entry_missing_optional_fields() {
        let entry = serde_json::json!(["/m/0abc", null, null, null, null, null]);
        // place_id is present; everything else is None/default.
        let result = parse_destination_entry(entry).unwrap();
        assert_eq!(result.place_id, "/m/0abc");
        assert!(result.name.is_empty());
        assert!(result.date_from.is_none());
    }

    #[test]
    fn parse_destination_entry_empty_place_id_errors() {
        let entry = serde_json::json!(["", "Paris"]);
        assert!(parse_destination_entry(entry).is_err());
    }

    #[test]
    fn looks_like_destination_list_positive() {
        let arr: Vec<Value> = vec![serde_json::json!(["/m/0vzm", "Vienna"])];
        assert!(looks_like_destination_list(&arr));
    }

    #[test]
    fn looks_like_destination_list_negative() {
        let arr: Vec<Value> = vec![serde_json::json!(42)];
        assert!(!looks_like_destination_list(&arr));
    }
}
