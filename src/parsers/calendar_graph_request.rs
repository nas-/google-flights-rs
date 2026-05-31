use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use percent_encoding::utf8_percent_encode;

use crate::parsers::common::{FlightTimes, StopoverDuration, TotalDuration};

use super::common::{
    Location, RequestBody, SerializeToWeb, SortOrder, StopOptions, ToRequestBody, TravelClass,
    Travelers,
};
use super::{common::CHARACTERS_TO_ENCODE, flight_request::ItineraryRequest};
use crate::parsers::constants::CALENDAR_GRAPH;

use anyhow::Result;

pub struct GraphRequestOptions<'a> {
    pub departing_city: &'a [Location],
    pub arriving_city: &'a [Location],
    pub date_start: &'a NaiveDate,
    pub date_return: Option<&'a NaiveDate>,
    pub date_end_graph: &'a str,
    pub travellers: Travelers,
    pub travel_class: &'a TravelClass,
    pub stop_option: &'a StopOptions,
    pub departing_times: &'a FlightTimes,
    pub return_times: &'a FlightTimes,
    pub stopover_max: &'a StopoverDuration,
    pub duration_max: &'a TotalDuration,
    pub frontend_version: &'a String,
    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`.
    pub language: &'a str,
    /// ISO 3166-1 alpha-2 country code (upper-case), e.g. `"GB"`.
    pub country: &'a str,
    /// Result sort order sent to Google Flights.
    pub sort_order: &'a SortOrder,
}

impl ToRequestBody for GraphRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&GraphRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;
    fn try_from(options: &GraphRequestOptions) -> Result<Self> {
        let date_start = options.date_start.to_string();
        let binding = options.date_return.map(|f| f.to_string());
        let date_return = binding.as_deref();
        let itinerary = ItineraryRequest::new(
            options.departing_city,
            options.arriving_city,
            options.stop_option,
            &date_start,
            &date_return,
            &options.travellers,
            options.travel_class,
            options.departing_times,
            options.return_times,
            options.stopover_max,
            options.duration_max,
            true,
            *options.sort_order,
        );
        let graph_req = GraphRequest {
            itinerary,
            date_start_graph: &options.date_start.to_string(),
            date_end_graph: options.date_end_graph,
        };
        let body = graph_req.serialize_to_web()?;

        let url = format!(
            "{CALENDAR_GRAPH}?f.sid=-8880820772586824788&bl={}&hl={}-{}&soc-app=162&soc-platform=1&soc-device=1&_reqid=957285&rt=c",
            options.frontend_version,
            options.language,
            options.country.to_uppercase()
        );

        Ok(Self {
            url,
            body: utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string(),
        })
    }
}

struct GraphRequest<'a> {
    itinerary: ItineraryRequest<'a>,
    date_start_graph: &'a str,
    date_end_graph: &'a str,
}

impl SerializeToWeb for GraphRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let dates: Vec<&str> = self.itinerary.legs.iter().map(|f| f.date).collect();

        let diff_days: String = if dates.len() == 2 {
            let date1 = NaiveDate::parse_from_str(dates[0], "%Y-%m-%d")?;
            let date2 = NaiveDate::parse_from_str(dates[1], "%Y-%m-%d")?;

            let diff_dates = date2.signed_duration_since(date1).num_days();
            format!(",null,[{0},{0}]", diff_dates)
        } else {
            "".to_string()
        };

        Ok(format!(
            r#"f.req=[null,"[null,{0},[\"{1}\",\"{2}\"]{3}]"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:{4}&"#,
            self.itinerary.serialize_to_web()?,
            self.date_start_graph,
            self.date_end_graph,
            diff_days,
            epoch_now
        ))
    }
}

#[cfg(test)]
mod tests {

    use std::vec;

    use crate::parsers::common::PlaceType;
    use anyhow::Ok;

    use super::*;

    #[test]
    fn test_produce_correct_body() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0]).unwrap();
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "SYD".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let binding: FlightTimes = FlightTimes::default();
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let date_start = NaiveDate::parse_from_str("2024-02-02", "%Y-%m-%d").unwrap();
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let search_settings = GraphRequestOptions {
            departing_city: core::slice::from_ref(&departure),
            arriving_city: core::slice::from_ref(&arrival),
            date_start: &date_start,
            date_return: None,
            date_end_graph: "2024-05-02",
            travellers,
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &binding,
            return_times: &binding,
            stopover_max: &stopover_max,
            duration_max: &duration_max,
            frontend_version: &frontend_version,
            language: "en",
            country: "GB",
            sort_order: &SortOrder::Best,
        };

        let req: RequestBody = (&search_settings).try_into()?;
        let expected = "f.req=%5Bnull%2C%22%5Bnull%2C%5Bnull%2Cnull%2C1%2Cnull%2C%5B%5D%2C1%2C%5B1%2C0%2C0%2C0%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-02-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%5D%2Cnull%2Cnull%2Cnull%2C1%2C1%5D%2C%5B%5C%222024-02-02%5C%22%2C%5C%222024-05-02%5C%22%5D%5D%22%5D&";
        assert!(req.body.starts_with(expected));
        Ok(())
    }

    #[test]
    fn test_produce_correct_parser() -> Result<()> {
        let travelers = Travelers::new([1, 0, 0, 0].to_vec()).unwrap();
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "SYD".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let flight_times = FlightTimes::default();
        let itinerary = ItineraryRequest::new(
            core::slice::from_ref(&departure),
            core::slice::from_ref(&arrival),
            &StopOptions::All,
            "2024-02-02",
            &None,
            &travelers,
            &TravelClass::Economy,
            &flight_times,
            &flight_times,
            &stopover_max,
            &duration_max,
            true,
            SortOrder::Best,
        );

        let x = GraphRequest {
            itinerary,
            date_start_graph: "2024-02-02",
            date_end_graph: "2024-05-02",
        };

        let expected = r#"f.req=[null,"[null,[null,null,1,null,[],1,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"SYD\",0]]],null,0,null,null,\"2024-02-02\",null,null,null,null,null,null,null,3]],null,null,null,1,1],[\"2024-02-02\",\"2024-05-02\"]]"]"#;
        assert!(x.serialize_to_web()?.starts_with(expected));
        Ok(())
    }
}
