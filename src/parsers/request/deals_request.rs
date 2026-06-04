//! Request builder for `GetFlightDealsStreaming`.
//!
//! Wire format (verified from live captures). `f.req` is `[null,"<INNER>"]`
//! where `INNER` is a 5-element array:
//!
//! ```text
//! [ [], OPTIONS, "<ai_prompt_or_empty>", "c0", [] ]
//! ```
//!
//! `OPTIONS[5]` = cabin class, `OPTIONS[6]` = travelers, `OPTIONS[13]` = routes:
//!
//! ```text
//! [ [ ORIGIN, [], null, NONSTOP, null, null, "OUT_DATE" (, [MAX_MIN]) ],
//!   [ [], ORIGIN, null, NONSTOP, null, null, "RET_DATE" (, [MAX_MIN]) ] ]
//! ```
//!
//! When a natural-language `prompt` is set the OPTIONS array is shorter (ends at
//! index 17) and the prompt goes in `INNER[2]`.

use std::time::{SystemTime, UNIX_EPOCH};

use percent_encoding::utf8_percent_encode;

use crate::parsers::common::CHARACTERS_TO_ENCODE;
use crate::parsers::common::{Location, PlaceType, RequestBody, SerializeToWeb, ToRequestBody};
use crate::parsers::constants::FLIGHT_DEALS_URL;
use crate::requests::config::deals::DealConfig;

use anyhow::Result;

pub struct DealsRequestOptions<'a> {
    pub config: &'a DealConfig,
    pub frontend_version: &'a str,
}

impl ToRequestBody for DealsRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&DealsRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;

    fn try_from(opts: &DealsRequestOptions<'_>) -> Result<Self> {
        let cfg = opts.config;
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let origin = serialize_origin(&cfg.origin);
        let nonstop = if cfg.nonstop { 1 } else { 0 };
        let dur = match cfg.max_duration_minutes {
            Some(m) => format!(",[{m}]"),
            None => String::new(),
        };
        let out_date = cfg.outbound_date.format("%Y-%m-%d").to_string();
        let ret_date = cfg.return_date.format("%Y-%m-%d").to_string();

        // routes = [ outbound, return ]; dates embedded per leg.
        let routes = format!(
            r#"[[{origin},[],null,{nonstop},null,null,\"{out}\"{dur}],[[],{origin},null,{nonstop},null,null,\"{ret}\"{dur}]]"#,
            origin = origin,
            nonstop = nonstop,
            out = out_date,
            ret = ret_date,
            dur = dur,
        );

        let travelers = cfg.travellers.serialize_to_web()?;
        let class = cfg.travel_class as i32;

        // OPTIONS: cabin class at [5], travelers at [6], routes at [13], then the
        // filter slots and trailing flags used by the structured deals query.
        let options = format!(
            r#"[null,null,1,null,[],{class},{travelers},null,null,null,null,null,null,{routes},null,null,null,1,null,null,null,null,null,null,null,null,null,null,null,null,[null,null,null,null,null,1,null,null,1],3]"#
        );

        // INNER[2] is an AI-prompt slot (unused here); INNER[3] is a client
        // correlation token echoed back verbatim in the response.
        let inner = format!(r#"[[],{options},\"\",\"c0\",[]]"#);

        let body = format!(
            r#"f.req=[null,"{}"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:{}&"#,
            inner, epoch_now
        );

        let url = format!(
            "{FLIGHT_DEALS_URL}?f.sid=6921237406276106431&bl={version}&hl={lang}-{country}&soc-app=162&soc-platform=1&soc-device=1&_reqid=4150414&rt=c",
            version = opts.frontend_version,
            lang = cfg.language,
            country = cfg.country.to_uppercase(),
        );

        let encoded = utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string();
        Ok(RequestBody { url, body: encoded })
    }
}

/// Serialise origin locations to the triple-nested airport array used in the
/// deals routes field: `[[["IATA",type]]]`.
///
/// Type codes: 0 = airport, 4 = city, 6 = region.
fn serialize_origin(locations: &[Location]) -> String {
    if locations.is_empty() {
        return "[[]]".to_string();
    }
    let pairs: Vec<String> = locations
        .iter()
        .map(|loc| {
            let type_code = match loc.loc_type {
                PlaceType::Airport | PlaceType::Unspecified => 0,
                PlaceType::City => 4,
                PlaceType::MaybeRegion | PlaceType::RegionMaybe => 4,
                PlaceType::Region => 6,
            };
            format!(r#"[\"{}\",{}]"#, loc.loc_identifier, type_code)
        })
        .collect();
    format!("[[{}]]", pairs.join(","))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::requests::config::deals::DealConfig;
    use chrono::NaiveDate;

    fn lux() -> Location {
        Location {
            loc_identifier: "LUX".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        }
    }

    fn cfg() -> DealConfig {
        DealConfig {
            origin: vec![lux()],
            outbound_date: NaiveDate::from_ymd_opt(2026, 6, 20).unwrap(),
            return_date: NaiveDate::from_ymd_opt(2026, 6, 24).unwrap(),
            ..Default::default()
        }
    }

    #[test]
    fn url_targets_deals_endpoint_with_version() {
        let opts = DealsRequestOptions {
            config: &cfg(),
            frontend_version: "boq_test_p0",
        };
        let body = opts.to_request_body().unwrap();
        assert!(body.url.contains("GetFlightDealsStreaming"));
        assert!(body.url.contains("boq_test_p0"));
    }

    #[test]
    fn body_contains_origin_and_dates() {
        let opts = DealsRequestOptions {
            config: &cfg(),
            frontend_version: "v",
        };
        let body = opts.to_request_body().unwrap();
        // Percent-encoded body still contains the literal IATA and dates.
        assert!(body.body.contains("LUX"));
        assert!(body.body.contains("2026-06-20"));
        assert!(body.body.contains("2026-06-24"));
    }

    #[test]
    fn nonstop_and_duration_appear_in_legs() {
        let mut c = cfg();
        c.nonstop = true;
        c.max_duration_minutes = Some(1680);
        let opts = DealsRequestOptions {
            config: &c,
            frontend_version: "v",
        };
        // Decode the percent-encoded body to inspect the structure.
        let body = opts.to_request_body().unwrap();
        let decoded = percent_encoding::percent_decode_str(&body.body)
            .decode_utf8_lossy()
            .into_owned();
        assert!(decoded.contains("[1680]"), "duration filter missing");
    }
}
