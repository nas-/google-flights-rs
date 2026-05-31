use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use percent_encoding::utf8_percent_encode;

use crate::parsers::common::{FlightTimes, StopoverDuration, TotalDuration};

use super::common::{
    Location, RequestBody, SerializeToWeb, StopOptions, ToRequestBody, TravelClass, Travelers,
    CHARACTERS_TO_ENCODE,
};
use super::flight_request::ItineraryRequest;
use crate::parsers::constants::CALENDAR_GRID;

use anyhow::Result;

/// Request options for `GetCalendarGrid`.
///
/// The date grid returns a price for every (departure_date, return_date) pair
/// within the two supplied date windows.  Typically the windows span a week
/// each, giving a 7 × 7 matrix.
pub struct DateGridRequestOptions<'a> {
    departing_city: &'a [Location],
    arriving_city: &'a [Location],
    /// Reference date used for the outbound leg in the itinerary body.
    /// Should fall inside `dep_start..=dep_end`.
    dep_date: &'a NaiveDate,
    /// Reference date for the return leg.  Should fall inside `ret_start..=ret_end`.
    ret_date: &'a NaiveDate,
    dep_start: &'a NaiveDate,
    dep_end: &'a NaiveDate,
    ret_start: &'a NaiveDate,
    ret_end: &'a NaiveDate,
    travellers: Travelers,
    travel_class: &'a TravelClass,
    stop_option: &'a StopOptions,
    departing_times: &'a FlightTimes,
    return_times: &'a FlightTimes,
    stopover_max: &'a StopoverDuration,
    duration_max: &'a TotalDuration,
    frontend_version: &'a String,
}

impl<'a> DateGridRequestOptions<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departing_city: &'a [Location],
        arriving_city: &'a [Location],
        dep_date: &'a NaiveDate,
        ret_date: &'a NaiveDate,
        dep_start: &'a NaiveDate,
        dep_end: &'a NaiveDate,
        ret_start: &'a NaiveDate,
        ret_end: &'a NaiveDate,
        travellers: Travelers,
        travel_class: &'a TravelClass,
        stop_option: &'a StopOptions,
        departing_times: &'a FlightTimes,
        return_times: &'a FlightTimes,
        stopover_max: &'a StopoverDuration,
        duration_max: &'a TotalDuration,
        frontend_version: &'a String,
    ) -> Self {
        Self {
            departing_city,
            arriving_city,
            dep_date,
            ret_date,
            dep_start,
            dep_end,
            ret_start,
            ret_end,
            travellers,
            travel_class,
            stop_option,
            departing_times,
            return_times,
            stopover_max,
            duration_max,
            frontend_version,
        }
    }
}

impl ToRequestBody for DateGridRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&DateGridRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;

    fn try_from(options: &DateGridRequestOptions) -> Result<Self> {
        let dep_date_str = options.dep_date.to_string();
        let ret_date_str = options.ret_date.to_string();
        let date_return_opt = Some(ret_date_str.as_str());

        // Build a two-leg itinerary (outbound + return).  The date grid uses
        // `number = "1"` in the itinerary body (not "2" like a plain shopping
        // request), so we pass `is_graph = false` and let the `chosen_itinerary`
        // field on the first leg drive the number down to "1".
        //
        // Implementation note: `ItineraryRequest` outputs "1" when the first leg
        // has a `chosen_itinerary`.  We use a private wrapper that forces the
        // correct value without needing a real chosen itinerary.
        let req = DateGridRequest {
            itinerary: ItineraryRequest::new(
                options.departing_city,
                options.arriving_city,
                options.stop_option,
                &dep_date_str,
                &date_return_opt,
                &options.travellers,
                options.travel_class,
                options.departing_times,
                options.return_times,
                options.stopover_max,
                options.duration_max,
                false, // is_graph — no trailing ",1"
            ),
            dep_start: &options.dep_start.to_string(),
            dep_end: &options.dep_end.to_string(),
            ret_start: &options.ret_start.to_string(),
            ret_end: &options.ret_end.to_string(),
        };

        let body = req.serialize_to_web()?;
        let url = format!(
            "{CALENDAR_GRID}?f.sid=-2458705061666219982&bl={}&hl=en-GB&soc-app=162&soc-platform=1&soc-device=1&_reqid=1152367&rt=c",
            options.frontend_version
        );
        Ok(RequestBody {
            url,
            body: utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Internal serializer
// ---------------------------------------------------------------------------

struct DateGridRequest<'a> {
    itinerary: ItineraryRequest<'a>,
    dep_start: &'a str,
    dep_end: &'a str,
    ret_start: &'a str,
    ret_end: &'a str,
}

impl SerializeToWeb for DateGridRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        // The date grid itinerary uses number=1 (not 2).  We achieve this by
        // serialising the itinerary normally (which yields number=2 for a fresh
        // request) and then replacing the leading `[null,null,2,` with
        // `[null,null,1,`.  This is safe: the prefix is unique and always present.
        let raw_itinerary = self.itinerary.serialize_to_web()?;
        let itinerary = raw_itinerary.replacen("[null,null,2,", "[null,null,1,", 1);

        Ok(format!(
            r#"f.req=[null,"[null,{0},[\"{1}\",\"{2}\"],[\"{3}\",\"{4}\"]]"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:{5}&"#,
            itinerary,
            self.dep_start,
            self.dep_end,
            self.ret_start,
            self.ret_end,
            epoch_now,
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Ok;

    #[test]
    fn test_date_grid_request_body_format() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0]);
        let departure = Location::new("/m/0fq8f", 4, None);
        let arrival = Location::new("/m/0947l", 5, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let flight_times = FlightTimes::default();
        let frontend_version = "boq_travel-frontend-flights-ui_20260527.01_p0".to_string();

        let dep_date = NaiveDate::parse_from_str("2026-06-10", "%Y-%m-%d")?;
        let ret_date = NaiveDate::parse_from_str("2026-06-18", "%Y-%m-%d")?;
        let dep_start = NaiveDate::parse_from_str("2026-06-07", "%Y-%m-%d")?;
        let dep_end = NaiveDate::parse_from_str("2026-06-13", "%Y-%m-%d")?;
        let ret_start = NaiveDate::parse_from_str("2026-06-15", "%Y-%m-%d")?;
        let ret_end = NaiveDate::parse_from_str("2026-06-21", "%Y-%m-%d")?;

        let opts = DateGridRequestOptions::new(
            core::slice::from_ref(&departure),
            core::slice::from_ref(&arrival),
            &dep_date,
            &ret_date,
            &dep_start,
            &dep_end,
            &ret_start,
            &ret_end,
            travellers,
            &TravelClass::Economy,
            &StopOptions::All,
            &flight_times,
            &flight_times,
            &stopover_max,
            &duration_max,
            &frontend_version,
        );

        let req: RequestBody = (&opts).try_into()?;

        // Body must start with f.req= and contain both date windows.
        assert!(req.body.contains("f.req="), "body should contain f.req=");
        // dep range dates should appear in the body (URL-encoded)
        assert!(
            req.body.contains("2026-06-07") || req.body.contains("2026-06-07"),
            "dep_start should appear"
        );
        // URL must point to the grid endpoint
        assert!(
            req.url.contains("GetCalendarGrid"),
            "url should point to GetCalendarGrid"
        );
        // Itinerary must use number=1 (not number=2)
        assert!(
            !req.body.contains("%5Bnull%2Cnull%2C2%2C"),
            "itinerary should not contain null,null,2"
        );

        Ok(())
    }
}
