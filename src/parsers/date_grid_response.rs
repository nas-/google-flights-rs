use anyhow::Result;
use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{
    common::{decode_inner_object, decode_outer_object, get_idx},
    flight_response::{RawResponseContainer, RawResponseContainerVec},
};

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

    /// Returns all entries where the departure falls on `dep_weekday` and the
    /// return falls on `ret_weekday`.
    ///
    /// Pair this with time-of-day constraints in [`Config`]'s `departing_times`
    /// / `return_times` — those are applied server-side when fetching the grid,
    /// so the prices already reflect the time window you care about.
    ///
    /// # Example
    /// ```rust,ignore
    /// // All Friday-out / Sunday-back options, prices already filtered to
    /// // afternoon departures because Config had departing_times set.
    /// for entry in grid.filter_weekdays(Weekday::Fri, Weekday::Sun) {
    ///     println!("{} → {}  €{}", entry.departure_date, entry.return_date, entry.price);
    /// }
    /// ```
    pub fn filter_weekdays(
        &self,
        dep_weekday: Weekday,
        ret_weekday: Weekday,
    ) -> impl Iterator<Item = &DateGridEntry> {
        self.entries.iter().filter(move |e| {
            e.departure_date.weekday() == dep_weekday
                && e.return_date.weekday() == ret_weekday
        })
    }

    /// Returns the cheapest entry whose (departure weekday, return weekday)
    /// pair matches any element of `combos`, or `None` if there are no matches.
    ///
    /// Useful when you are flexible about the exact days but want to stay
    /// within a set of acceptable combinations.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Cheapest trip that leaves Friday or Saturday and returns Sunday or Monday.
    /// let best = grid.cheapest_for_weekdays(&[
    ///     (Weekday::Fri, Weekday::Sun),
    ///     (Weekday::Fri, Weekday::Mon),
    ///     (Weekday::Sat, Weekday::Sun),
    ///     (Weekday::Sat, Weekday::Mon),
    /// ]);
    /// ```
    pub fn cheapest_for_weekdays(&self, combos: &[(Weekday, Weekday)]) -> Option<&DateGridEntry> {
        self.entries
            .iter()
            .filter(|e| {
                combos.iter().any(|(d, r)| {
                    e.departure_date.weekday() == *d && e.return_date.weekday() == *r
                })
            })
            .min_by_key(|e| e.price)
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

    /// 2026-06-05 = Friday, 2026-06-06 = Saturday, 2026-06-07 = Sunday,
    /// 2026-06-08 = Monday.
    #[test]
    fn test_filter_weekdays() -> Result<()> {
        let payload = r#"[
            null,
            [
                ["2026-06-05", "2026-06-07", [[null, 99], "tok_fri_sun"], 1],
                ["2026-06-05", "2026-06-08", [[null, 80], "tok_fri_mon"], 1],
                ["2026-06-06", "2026-06-07", [[null, 60], "tok_sat_sun"], 1],
                ["2026-06-07", "2026-06-08", [[null, 40], "tok_sun_mon"], 1]
            ]
        ]"#;
        let entries = parse_inner_payload(payload)?;
        let response = DateGridResponse { entries };

        let fri_sun: Vec<_> = response.filter_weekdays(Weekday::Fri, Weekday::Sun).collect();
        assert_eq!(fri_sun.len(), 1);
        assert_eq!(fri_sun[0].price, 99);

        let sat_sun: Vec<_> = response.filter_weekdays(Weekday::Sat, Weekday::Sun).collect();
        assert_eq!(sat_sun.len(), 1);
        assert_eq!(sat_sun[0].price, 60);

        Ok(())
    }

    #[test]
    fn test_cheapest_for_weekdays() -> Result<()> {
        let payload = r#"[
            null,
            [
                ["2026-06-05", "2026-06-07", [[null, 99], "tok_fri_sun"], 1],
                ["2026-06-05", "2026-06-08", [[null, 80], "tok_fri_mon"], 1],
                ["2026-06-06", "2026-06-07", [[null, 60], "tok_sat_sun"], 1],
                ["2026-06-07", "2026-06-08", [[null, 40], "tok_sun_mon"], 1]
            ]
        ]"#;
        let entries = parse_inner_payload(payload)?;
        let response = DateGridResponse { entries };

        // Sat→Sun (60) is cheapest among the four weekend combos; Sun→Mon (40)
        // is excluded because it is not in the combo list.
        let best = response.cheapest_for_weekdays(&[
            (Weekday::Fri, Weekday::Sun),
            (Weekday::Fri, Weekday::Mon),
            (Weekday::Sat, Weekday::Sun),
            (Weekday::Sat, Weekday::Mon),
        ]);
        assert_eq!(best.map(|e| e.price), Some(60));

        // No match → None
        let none = response.cheapest_for_weekdays(&[(Weekday::Wed, Weekday::Thu)]);
        assert!(none.is_none());

        Ok(())
    }
}
