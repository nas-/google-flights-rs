use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

use crate::parsers::common::{decode_inner_object, decode_outer_object, get_idx};
use crate::parsers::response::flight_response::{RawResponseContainer, RawResponseContainerVec};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One cell in the date grid: a (departure_date, return_date) pair with its
/// cheapest available price and an opaque booking token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateGridEntry {
    pub departure_date: NaiveDate,
    pub return_date: NaiveDate,
    /// Price in the currency requested (e.g. 82 for €82).
    pub price: i32,
    /// Opaque booking token that can be used to construct a deep link.
    pub booking_token: Option<String>,
}

/// Parsed response from `GetCalendarGrid`.
///
/// Contains a flat list of [`DateGridEntry`] values covering all
/// (departure_date × return_date) combinations that Google returned.
#[derive(Debug, Serialize, Deserialize)]
pub struct DateGridResponse {
    pub entries: Vec<DateGridEntry>,
}

impl DateGridResponse {
    /// Returns the entry with the lowest price, or `None` if empty.
    pub fn cheapest(&self) -> Option<&DateGridEntry> {
        self.entries.iter().min_by_key(|e| e.price)
    }

    /// Returns a nested map `dep_date → ret_date → price` for easy grid lookup.
    pub fn grid(&self) -> HashMap<NaiveDate, HashMap<NaiveDate, i32>> {
        let mut map: HashMap<NaiveDate, HashMap<NaiveDate, i32>> = HashMap::new();
        for e in &self.entries {
            map.entry(e.departure_date)
                .or_default()
                .insert(e.return_date, e.price);
        }
        map
    }
}

/// Renders the date grid as an ASCII price table.
///
/// Rows are departure dates, columns are return dates, cells are prices.
/// Missing combinations show `-`.
///
/// ```text
/// dep \ ret      06-15   06-16   06-17   06-18
/// 06-07            84      51      46      31
/// 06-08            82      50      44      30
/// ```
impl fmt::Display for DateGridResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let grid = self.grid();

        let mut dep_dates: Vec<NaiveDate> = grid.keys().copied().collect();
        dep_dates.sort();

        let mut ret_dates: Vec<NaiveDate> = grid
            .values()
            .flat_map(|m| m.keys().copied())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        ret_dates.sort();

        // Header
        write!(f, "{:<12}", "dep \\ ret")?;
        for r in &ret_dates {
            write!(f, "{:>8}", r.format("%m-%d").to_string())?;
        }
        writeln!(f)?;

        // Rows
        for dep in &dep_dates {
            write!(f, "{:<12}", dep.format("%m-%d").to_string())?;
            for ret in &ret_dates {
                let cell = grid
                    .get(dep)
                    .and_then(|m| m.get(ret))
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());
                write!(f, "{:>8}", cell)?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a raw `GetCalendarGrid` HTTP response body into a [`DateGridResponse`].
pub fn parse_date_grid_response(raw: &str) -> Result<DateGridResponse> {
    let outer: Vec<RawResponseContainerVec> = decode_outer_object(raw)?;
    let entries: Vec<DateGridEntry> = outer
        .iter()
        .flat_map(|f| &f.resp)
        .filter_map(|r: &RawResponseContainer| r.payload.as_deref())
        .filter_map(|payload| parse_inner_payload(payload).ok())
        .flatten()
        .collect();
    Ok(DateGridResponse { entries })
}

/// Parse one inner payload string into a list of grid entries.
///
/// Inner payload structure (index-based):
/// ```text
/// arr[0] = response metadata
/// arr[1] = [[dep_date, ret_date, [[null, price], token], 1], ...]
/// ```
fn parse_inner_payload(payload: &str) -> Result<Vec<DateGridEntry>> {
    let arr: Vec<Value> = decode_inner_object(payload)?;
    let raw_entries: Vec<Value> = get_idx(&arr, 1).unwrap_or_default();

    let entries = raw_entries
        .into_iter()
        .filter_map(|v| parse_entry(v).ok())
        .collect();
    Ok(entries)
}

/// Parse one grid entry: `["dep_date", "ret_date", [[null, price], token], 1]`
fn parse_entry(v: Value) -> Result<DateGridEntry> {
    let arr = match v.as_array() {
        Some(a) => a.clone(),
        None => anyhow::bail!("entry is not an array"),
    };

    let dep_str: String = get_idx(&arr, 0).ok_or_else(|| anyhow::anyhow!("missing dep_date"))?;
    let ret_str: String = get_idx(&arr, 1).ok_or_else(|| anyhow::anyhow!("missing ret_date"))?;

    let departure_date = NaiveDate::parse_from_str(&dep_str, "%Y-%m-%d")?;
    let return_date = NaiveDate::parse_from_str(&ret_str, "%Y-%m-%d")?;

    // arr[2] = [[null, price], booking_token]
    let price_arr: Vec<Value> = get_idx(&arr, 2).unwrap_or_default();
    let price: i32 = price_arr
        .first()
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .ok_or_else(|| anyhow::anyhow!("missing price"))?;
    let booking_token: Option<String> = price_arr
        .get(1)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(DateGridEntry {
        departure_date,
        return_date,
        price,
        booking_token,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Minimal synthetic inner payload — two entries, one cheaper.
    #[test]
    fn test_parse_inner_payload() -> Result<()> {
        let payload = r#"[
            [null, "metadata"],
            [
                ["2026-06-07", "2026-06-15", [[null, 44], "tok_a"], 1],
                ["2026-06-08", "2026-06-16", [[null, 82], "tok_b"], 1]
            ]
        ]"#;

        let entries = parse_inner_payload(payload)?;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].price, 44);
        assert_eq!(entries[1].price, 82);
        assert_eq!(
            entries[0].departure_date,
            NaiveDate::from_ymd_opt(2026, 6, 7).unwrap()
        );
        assert_eq!(
            entries[0].return_date,
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()
        );
        assert_eq!(entries[0].booking_token.as_deref(), Some("tok_a"));
        Ok(())
    }

    #[test]
    fn test_cheapest() -> Result<()> {
        let payload = r#"[
            null,
            [
                ["2026-06-07", "2026-06-15", [[null, 44], "tok_a"], 1],
                ["2026-06-08", "2026-06-16", [[null, 82], "tok_b"], 1],
                ["2026-06-09", "2026-06-17", [[null, 31], "tok_c"], 1]
            ]
        ]"#;
        let entries = parse_inner_payload(payload)?;
        let response = DateGridResponse { entries };
        let cheapest = response.cheapest().expect("should have cheapest");
        assert_eq!(cheapest.price, 31);
        assert_eq!(cheapest.booking_token.as_deref(), Some("tok_c"));
        Ok(())
    }

    #[test]
    fn test_grid_structure() -> Result<()> {
        let payload = r#"[
            null,
            [
                ["2026-06-07", "2026-06-15", [[null, 44], "tok_a"], 1],
                ["2026-06-07", "2026-06-16", [[null, 55], "tok_b"], 1],
                ["2026-06-08", "2026-06-15", [[null, 66], "tok_c"], 1]
            ]
        ]"#;
        let entries = parse_inner_payload(payload)?;
        let response = DateGridResponse { entries };
        let grid = response.grid();

        let dep_07 = NaiveDate::from_ymd_opt(2026, 6, 7).unwrap();
        let dep_08 = NaiveDate::from_ymd_opt(2026, 6, 8).unwrap();
        let ret_15 = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let ret_16 = NaiveDate::from_ymd_opt(2026, 6, 16).unwrap();

        assert_eq!(grid[&dep_07][&ret_15], 44);
        assert_eq!(grid[&dep_07][&ret_16], 55);
        assert_eq!(grid[&dep_08][&ret_15], 66);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // DateGridResponse Display (ASCII table)
    // -----------------------------------------------------------------------

    fn make_entry(dep: (i32, u32, u32), ret: (i32, u32, u32), price: i32) -> DateGridEntry {
        DateGridEntry {
            departure_date: NaiveDate::from_ymd_opt(dep.0, dep.1, dep.2).unwrap(),
            return_date: NaiveDate::from_ymd_opt(ret.0, ret.1, ret.2).unwrap(),
            price,
            booking_token: None,
        }
    }

    #[test]
    fn test_display_contains_header_and_prices() {
        let response = DateGridResponse {
            entries: vec![
                make_entry((2026, 6, 7), (2026, 6, 15), 84),
                make_entry((2026, 6, 7), (2026, 6, 16), 51),
                make_entry((2026, 6, 8), (2026, 6, 15), 82),
                make_entry((2026, 6, 8), (2026, 6, 16), 50),
            ],
        };

        let output = format!("{response}");

        // Header
        assert!(output.contains("dep \\ ret"), "should contain header label");
        assert!(
            output.contains("06-15"),
            "should contain return-date column 06-15"
        );
        assert!(
            output.contains("06-16"),
            "should contain return-date column 06-16"
        );

        // Row labels
        assert!(
            output.contains("06-07"),
            "should contain departure-date row 06-07"
        );
        assert!(
            output.contains("06-08"),
            "should contain departure-date row 06-08"
        );

        // Prices
        assert!(output.contains("84"), "should contain price 84");
        assert!(output.contains("51"), "should contain price 51");
        assert!(output.contains("82"), "should contain price 82");
        assert!(output.contains("50"), "should contain price 50");
    }

    #[test]
    fn test_display_missing_cell_shows_dash() {
        // dep 06-07 has only ret 06-15; dep 06-08 has only ret 06-16
        // → the (06-07, 06-16) and (06-08, 06-15) cells are missing → "-"
        let response = DateGridResponse {
            entries: vec![
                make_entry((2026, 6, 7), (2026, 6, 15), 100),
                make_entry((2026, 6, 8), (2026, 6, 16), 200),
            ],
        };

        let output = format!("{response}");
        assert!(
            output.contains('-'),
            "missing grid cells should render as '-'"
        );
    }

    #[test]
    fn test_display_empty_grid() {
        let response = DateGridResponse { entries: vec![] };
        let output = format!("{response}");
        // Empty: just the header with no data columns
        assert!(
            output.contains("dep \\ ret"),
            "empty grid should still have header"
        );
    }

    #[test]
    fn test_display_single_cell() {
        let response = DateGridResponse {
            entries: vec![make_entry((2026, 8, 1), (2026, 8, 10), 999)],
        };

        let output = format!("{response}");
        assert!(output.contains("08-01"), "single-cell dep date");
        assert!(output.contains("08-10"), "single-cell ret date");
        assert!(output.contains("999"), "single-cell price");
    }
}
