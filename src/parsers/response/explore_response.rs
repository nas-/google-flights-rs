//! Response parser for `GetExploreDestinations`.
//!
//! # Wire format (verified from live captures)
//!
//! The HTTP body is a streaming JSONL payload:
//! - Line 0: `)]}'`
//! - Line 1: (empty)
//! - Line 2: byte-count for next data line
//! - Line 3: `[["wrb.fr", null, "INNER_JSON"]]`  ← chunk 1 (destinations)
//! - Line 4: byte-count for next data line
//! - Line 5: `[["wrb.fr", null, "INNER_JSON"]]`  ← chunk 2 (flight details + prices)
//!
//! **Chunk 1** inner JSON — top-level array:
//! - `[0]`: session metadata
//! - `[1]`: null
//! - `[2]`: map bounding box
//! - `[3]`: `[ [dest_entries...] ]`  ← destinations list (1-element wrapper)
//! - `[4]`: null
//! - `[5]`: UI filters metadata (alliances, interests)
//! - `[6]`: origin airport info
//!
//! Each destination entry (from `chunk1[3][0]`):
//! - `[0]`  place_id (`/m/…` or `/g/…`)
//! - `[1]`  coords `[lat, lng]`
//! - `[2]`  name
//! - `[3]`  image_url
//! - `[4]`  country
//! - `[5]`  type flag (1=city, 2=region)
//! - `[11]` date_from (`YYYY-MM-DD`)
//! - `[12]` date_to
//! - `[15]` nearest_airport IATA
//! - `[17]` flight_duration_minutes (or null)
//! - `[22]` stops (or null)
//! - `[27]` Google Places ID (not the booking token)
//!
//! **Chunk 2** inner JSON — top-level array:
//! - `[4]`: flight-detail entries, one per destination
//!
//! Each flight-detail entry (from `chunk2[4]`):
//! - `[0]`  place_id
//! - `[1]`  `[[null, price_eur], "booking_token_b64"]` or null
//! - `[6]`  `["airline_code", "airline_name", stops, duration_mins, null, "dest_iata", ...]`
//! - `[15]` `[[null, accommodation_price]]` (optional)

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;
use std::collections::HashMap;

use crate::parsers::common::{
    decode_inner_object, decode_outer_object, get_idx, GetOuterErrorMessages,
};
use crate::requests::config::explore::ExploreResult;

// ---------------------------------------------------------------------------
// Raw outer-wrapper types
// ---------------------------------------------------------------------------

// Each response line: [["wrb.fr", null, payload_str, ...], ["di", ...], ...]
// Outer array element [0] is the wrb.fr entry; payload is at position [2] within it.

#[derive(Debug)]
pub(crate) struct ExploreRawChunk {
    pub payload: Option<String>,
}

impl<'de> serde::Deserialize<'de> for ExploreRawChunk {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let outer = Vec::<serde_json::Value>::deserialize(d)?;
        let wrb_entry: Vec<serde_json::Value> = crate::parsers::common::get_idx(&outer, 0)
            .ok_or_else(|| serde::de::Error::custom("missing wrb.fr entry at [0]"))?;
        let payload: Option<String> = crate::parsers::common::get_idx(&wrb_entry, 2);
        Ok(ExploreRawChunk { payload })
    }
}

impl GetOuterErrorMessages for ExploreRawChunk {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        None
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
    let chunks: Vec<ExploreRawChunk> = match decode_outer_object(raw) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "explore: failed to decode outer object");
            return Ok(vec![]);
        }
    };

    if chunks.is_empty() {
        tracing::debug!("explore: no chunks found");
        return Ok(vec![]);
    }

    // Use a HashMap keyed by place_id to merge chunk1 (destinations) and chunk2 (prices).
    let mut destinations: HashMap<String, ExploreResult> = HashMap::new();

    for chunk in &chunks {
        let payload = match chunk.payload.as_deref() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };

        let arr: Vec<Value> = match decode_inner_object(payload) {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(error = %e, "explore: failed to decode inner payload");
                continue;
            }
        };

        // ── Chunk 1: destinations at arr[3][0] ──────────────────────────────
        // arr[3] = [[dest1, dest2, ...]]  (1-element outer wrapper)
        if let Some(outer) = arr.get(3).and_then(|v| v.as_array()) {
            if let Some(dest_list) = outer.first().and_then(|v| v.as_array()) {
                for entry in dest_list {
                    if let Ok(dest) = parse_destination_entry(entry.clone()) {
                        destinations.entry(dest.place_id.clone()).or_insert(dest);
                    }
                }
            }
        }

        // ── Chunk 2: flight details at arr[4][0] (double-wrapped like chunk 1) ──
        if let Some(detail_list) = arr
            .get(4)
            .and_then(|v| v.as_array())
            .and_then(|outer| outer.first())
            .and_then(|v| v.as_array())
        {
            for entry in detail_list {
                let entry_arr = match entry.as_array() {
                    Some(a) => a,
                    None => continue,
                };

                let place_id: String = match get_idx(entry_arr, 0) {
                    Some(s) => s,
                    None => continue,
                };

                let dest = match destinations.get_mut(&place_id) {
                    Some(d) => d,
                    None => continue,
                };

                // [1] = [[null, price_eur], "booking_token_b64"] or null
                if let Some(price_info) = entry_arr.get(1).and_then(|v| v.as_array()) {
                    // price_info[0] = [null, price_eur]
                    if let Some(price_pair) = price_info.first().and_then(|v| v.as_array()) {
                        dest.price = get_idx(price_pair, 1);
                    }
                    // price_info[1] = opaque booking token (base64)
                    if let Some(tok) = get_idx::<String>(price_info, 1) {
                        dest.booking_token = tok;
                    }
                }

                // [6] = ["airline_code", "airline_name", stops, duration_mins, ...]
                if let Some(flight_detail) = get_idx::<Vec<Value>>(entry_arr, 6) {
                    dest.airline = get_idx(&flight_detail, 0);
                    if let Some(s) = get_idx::<i64>(&flight_detail, 2) {
                        dest.stops = Some(s.clamp(0, 255) as u8);
                    }
                    if let Some(d) = get_idx::<i64>(&flight_detail, 3) {
                        dest.flight_duration_minutes = Some(d.max(0) as u32);
                    }
                }

                // [15] = [[null, accommodation_price_nightly]]
                if let Some(acc_outer) = get_idx::<Vec<Value>>(entry_arr, 15) {
                    if let Some(acc_pair) = acc_outer.first().and_then(|v| v.as_array()) {
                        dest.accommodation_price = get_idx(acc_pair, 1);
                    }
                }
            }
        }
    }

    Ok(destinations.into_values().collect())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse one destination entry from chunk 1 (`arr[3][0][n]`).
///
/// Wire positions (verified from live captures):
/// ```text
/// [0]  place_id       String ("/m/…" or "/g/…")
/// [1]  coords         [lat: f64, lng: f64]
/// [2]  name           String
/// [3]  image_url      String or null
/// [4]  country        String
/// [11] date_from      String "YYYY-MM-DD" or null
/// [12] date_to        String "YYYY-MM-DD" or null
/// [15] nearest_airport String (IATA)
/// [17] flight_duration_minutes  i64 or null
/// [22] stops          i64 or null
/// [27] google_places_id  String or null (NOT the booking token)
/// ```
fn parse_destination_entry(v: Value) -> Result<ExploreResult> {
    let arr = match v {
        Value::Array(a) => a,
        _ => anyhow::bail!("destination entry is not an array"),
    };

    let place_id: String = get_idx(&arr, 0).unwrap_or_default();
    if place_id.is_empty() {
        anyhow::bail!("destination entry has empty place_id");
    }

    let coords_arr: Vec<f64> = get_idx(&arr, 1).unwrap_or_default();
    let coords = (
        coords_arr.first().copied().unwrap_or(0.0),
        coords_arr.get(1).copied().unwrap_or(0.0),
    );

    let name: String = get_idx(&arr, 2).unwrap_or_default();
    let image_url: Option<String> = get_idx(&arr, 3);
    let country: String = get_idx(&arr, 4).unwrap_or_default();
    let nearest_airport: String = get_idx(&arr, 15).unwrap_or_default();

    let date_from: Option<NaiveDate> = get_idx::<String>(&arr, 11)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let date_to: Option<NaiveDate> = get_idx::<String>(&arr, 12)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    // stops and flight_duration_minutes come from chunk 2 (the cheapest-flight detail).
    // Chunk 1 positions [22] and [17] can reflect different routes — don't use them.

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
        booking_token: String::new(), // filled in from chunk 2
    })
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
        // Construct a 28-element entry matching the wire format.
        let mut v = vec![serde_json::Value::Null; 29];
        v[0] = serde_json::json!("/m/0vzm");
        v[1] = serde_json::json!([48.2082, 16.3738]);
        v[2] = serde_json::json!("Vienna");
        v[3] = serde_json::json!("https://example.com/img.jpg");
        v[4] = serde_json::json!("Austria");
        v[11] = serde_json::json!("2026-08-01");
        v[12] = serde_json::json!("2026-08-08");
        v[15] = serde_json::json!("VIE");
        v[17] = serde_json::json!(120);
        v[22] = serde_json::json!(0);
        let result = parse_destination_entry(Value::Array(v)).unwrap();
        assert_eq!(result.place_id, "/m/0vzm");
        assert_eq!(result.name, "Vienna");
        assert_eq!(result.country, "Austria");
        assert!((result.coords.0 - 48.2082).abs() < 1e-3);
        assert_eq!(result.nearest_airport, "VIE");
        assert_eq!(result.flight_duration_minutes, None); // set from chunk 2
        assert_eq!(result.stops, None); // set from chunk 2
        assert!(result.date_from.is_some());
        assert!(result.date_to.is_some());
    }

    #[test]
    fn parse_destination_entry_empty_place_id_errors() {
        let entry = serde_json::json!(["", "Paris"]);
        assert!(parse_destination_entry(entry).is_err());
    }
}
