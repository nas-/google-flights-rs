//! Request builder for `GetExploreDestinations`.
//!
//! Wire format: the `f.req` inner JSON is a 12-element root array; element \[3\]
//! is an 18-element OPTIONS array encoding all search parameters.

use std::time::{SystemTime, UNIX_EPOCH};

use percent_encoding::utf8_percent_encode;

use crate::parsers::common::{RequestBody, SerializeToWeb, ToRequestBody, CHARACTERS_TO_ENCODE};
use crate::parsers::constants::EXPLORE_URL;
use crate::requests::config::explore::ExploreConfig;

use anyhow::Result;

pub struct ExploreRequestOptions<'a> {
    pub config: &'a ExploreConfig,
    pub frontend_version: &'a str,
}

impl ToRequestBody for ExploreRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&ExploreRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;

    fn try_from(opts: &ExploreRequestOptions<'_>) -> Result<Self> {
        let cfg = opts.config;
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        // ---------------------------------------------------------------------------
        // Build the routes array (OPTIONS[13])
        //
        // outbound: [[[[IATA, type_int]]], [], null, 0]
        // return:   [[], [[[IATA, type_int]]], null, 0]
        //
        // We need to serialize origin locations the same way SingleLegStruct does,
        // but adapted to the simpler explore format.
        // ---------------------------------------------------------------------------
        let origin_airports = serialize_origin_airports(&cfg.origin);

        // outbound: [ origin_airports, [], null, 0 ]
        // return:   [ [ [], origin_airports, null, 0 ] ]   ← return is double-wrapped
        // where origin_airports = [[["IATA",type_int]]]  (triple-nested per wire spec)
        let routes = format!(r#"[[{0},[],null,0],[[[],{0},null,0]]]"#, origin_airports);

        // ---------------------------------------------------------------------------
        // Trip date: [] or [month, duration_code]
        // ---------------------------------------------------------------------------
        let trip_date = match &cfg.trip_date {
            None => "[]".to_string(),
            Some(d) => {
                format!("[{},{}]", d.month, cfg.trip_duration.as_wire_code())
            }
        };
        // When no date is specified, still send the duration code in a wrapper or
        // keep as empty. The browser sends [] for "any date".
        // If date IS specified, the format is [month, duration_code].
        // When date is None but we still want to send duration (no month), use []:
        // The wire format spec shows [month_1..12, duration_code]. If month is not
        // specified, we send [].
        // Note: this is consistent with the spec above.

        // ---------------------------------------------------------------------------
        // Travelers: [adults, children, infants_lap, infants_seat]
        // ---------------------------------------------------------------------------
        let travelers = cfg.travellers.serialize_to_web()?;

        // ---------------------------------------------------------------------------
        // Travel class (cabin_class): 1=economy, 2=premium, 3=business, 4=first
        // ---------------------------------------------------------------------------
        let cabin_class = cfg.travel_class as i32;

        // ---------------------------------------------------------------------------
        // Optional filters
        // ---------------------------------------------------------------------------
        let price_limit = match cfg.max_price {
            None => "null".to_string(),
            Some(p) => format!("[null,{}]", p),
        };

        let alliance = match &cfg.airline_alliance {
            None => "null".to_string(),
            Some(a) => format!(r#"[\"{}\"]"#, a.as_google_str()),
        };

        let duration_limit = match cfg.max_flight_duration_minutes {
            None => "null".to_string(),
            Some(m) => format!("[{}]", m),
        };

        let baggage = match cfg.baggage {
            None => "null".to_string(),
            Some((carry_on, checked)) => format!("[{},{}]", carry_on, checked),
        };

        // interest_mid is handled inline in the options format below

        // ---------------------------------------------------------------------------
        // Map bounds (root [1] and [2])
        // ---------------------------------------------------------------------------
        let map_sw = match &cfg.map_bounds {
            None => "null".to_string(),
            Some(b) => format!("[{},{}]", b.sw.0, b.sw.1),
        };
        let map_ne = match &cfg.map_bounds {
            None => "null".to_string(),
            Some(b) => format!("[{},{}]", b.ne.0, b.ne.1),
        };

        // ---------------------------------------------------------------------------
        // trip_type at root [11]: 1=one_way, 2=round_trip
        // Explore always uses round_trip (2) since it searches for cheapest
        // return trip prices.
        // ---------------------------------------------------------------------------
        let trip_type = 2_i32;

        // ---------------------------------------------------------------------------
        // OPTIONS array (18 elements, indices 0–17)
        //
        // [0]  null
        // [1]  null
        // [2]  cabin_class
        // [3]  null
        // [4]  trip_date: [] or [month, duration_code]
        // [5]  1
        // [6]  travelers: [adults, children, infants_lap, infants_seat]
        // [7]  price_limit: null or [null, max_price_int]
        // [8]  alliance: null or ["ONEWORLD"]
        // [9]  duration_limit: null or [max_minutes_int]
        // [10] null
        // [11] baggage: null or [carry_on_count, checked_count]
        // [12] null
        // [13] routes: [[outbound], [return]]
        // [14] null
        // [15] null
        // [16] null
        // [17] interest_mid: null or MID string
        // ---------------------------------------------------------------------------
        // Base OPTIONS: 18 elements [0]-[17], last element is always 0.
        // Interest MID, when present, lives at position [27] with nulls filling [18]-[26].
        // Wire analysis from 16 captured requests.
        let options = if let Some(mid) = &cfg.interest {
            format!(
                r#"[null,null,{cabin_class},null,{trip_date},1,{travelers},{price_limit},{alliance},{duration_limit},null,{baggage},null,{routes},null,null,null,0,null,null,null,null,null,null,null,null,null,\"{mid}\"]"#
            )
        } else {
            format!(
                r#"[null,null,{cabin_class},null,{trip_date},1,{travelers},{price_limit},{alliance},{duration_limit},null,{baggage},null,{routes},null,null,null,0]"#
            )
        };

        // ---------------------------------------------------------------------------
        // Root 12-element array:
        // [0]  [] viewport
        // [1]  map SW corner [lat,lng] or null
        // [2]  map NE corner [lat,lng] or null
        // [3]  OPTIONS (18-element array)
        // [4]  null
        // [5]  1
        // [6]  null
        // [7]  0
        // [8]  null
        // [9]  1
        // [10] [1100,719] viewport size
        // [11] trip_type: 1=one_way, 2=round_trip
        // ---------------------------------------------------------------------------
        let inner = format!(
            r#"[[],{map_sw},{map_ne},{options},null,1,null,0,null,1,[1100,719],{trip_type}]"#
        );

        // f.req=[null,"12-element-array"] — the string value IS the 12-element array.
        let body = format!(
            r#"f.req=[null,"{}"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:{}&"#,
            inner, epoch_now
        );

        let url = format!(
            "{EXPLORE_URL}?f.sid=6921237406276106431&bl={version}&hl={lang}-{country}&soc-app=162&soc-platform=1&soc-device=1&_reqid=4150414&rt=c",
            version = opts.frontend_version,
            lang = cfg.language,
            country = cfg.country.to_uppercase(),
        );

        let encoded = utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string();
        Ok(RequestBody { url, body: encoded })
    }
}

/// Serialise origin locations to the nested airport array used in the routes
/// field of the explore request.
///
/// Each location is encoded as `["IATA", type_code]` where `type_code` comes
/// from `PlaceType` (0=airport, 4=city, 5=region).
/// Returns the triple-nested airport array used in the Explore routes field.
///
/// Each location becomes `[\"IATA\",type]`; the result is wrapped in three
/// array levels so the caller can slot it directly into the route structure:
///
/// ```text
/// outbound: [ origin_airports, [], null, 0 ]
/// return:   [ [[], origin_airports], null, 0 ]
/// ```
///
/// Wire spec: type 0 = airport, 4 = city/region (Google MID with 4-char prefix).
fn serialize_origin_airports(locations: &[crate::parsers::common::Location]) -> String {
    use crate::parsers::common::PlaceType;
    if locations.is_empty() {
        return "[[]]".to_string();
    }
    let pairs: Vec<String> = locations
        .iter()
        .map(|loc| {
            // Type 0 = airport, 4 = city (MID), 5 = region — always include the int.
            let type_code = match loc.loc_type {
                PlaceType::Airport | PlaceType::Unspecified => 0,
                PlaceType::City => 4,
                PlaceType::MaybeRegion | PlaceType::RegionMaybe => 4,
            };
            format!(r#"[\"{}\",{}]"#, loc.loc_identifier, type_code)
        })
        .collect();
    // Triple-nested: [[ [pair1], [pair2], ... ]]
    format!("[[{}]]", pairs.join(","))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::parsers::common::{Location, PlaceType};
    use crate::requests::config::explore::ExploreConfig;

    fn make_lux() -> Location {
        Location {
            loc_identifier: "LUX".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        }
    }

    #[test]
    fn explore_request_produces_url_with_frontend_version() {
        let cfg = ExploreConfig {
            origin: vec![make_lux()],
            ..Default::default()
        };
        let opts = ExploreRequestOptions {
            config: &cfg,
            frontend_version: "boq_travel-frontend-ui_20240110.02_p0",
        };
        let body = opts.to_request_body().unwrap();
        assert!(body.url.contains("boq_travel-frontend-ui_20240110.02_p0"));
        assert!(body.url.contains("GetExploreDestinations"));
    }

    #[test]
    fn explore_request_contains_origin_iata() {
        let cfg = ExploreConfig {
            origin: vec![make_lux()],
            ..Default::default()
        };
        let opts = ExploreRequestOptions {
            config: &cfg,
            frontend_version: "test",
        };
        let body = opts.to_request_body().unwrap();
        assert!(body.body.contains("LUX"), "body should contain LUX");
    }

    #[test]
    fn explore_request_with_max_price_contains_price() {
        let cfg = ExploreConfig {
            origin: vec![make_lux()],
            max_price: Some(300),
            ..Default::default()
        };
        let opts = ExploreRequestOptions {
            config: &cfg,
            frontend_version: "test",
        };
        let body = opts.to_request_body().unwrap();
        assert!(body.body.contains("300"), "body should contain max price");
    }
}
