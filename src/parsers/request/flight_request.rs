use std::{
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use percent_encoding::utf8_percent_encode;

use crate::parsers::common::{
    AirlineFilter, FixedFlights, FlightTimes, Location, RequestBody, SerializeToWeb, SortOrder,
    StopOptions, StopoverDuration, ToRequestBody, TotalDuration, TravelClass, Travelers,
    CHARACTERS_TO_ENCODE,
};
use crate::parsers::constants::{BOOKING_REQUEST, FLIGHT_REQUEST};
use crate::parsers::response::flight_response::FlightInfo;
use anyhow::Result;

pub struct FlightRequestOptions<'a> {
    pub departing_city: &'a [Location],
    pub arriving_city: &'a [Location],
    pub date_start: &'a str,
    pub date_return: Option<&'a str>,
    pub travellers: Travelers,
    pub travel_class: &'a TravelClass,
    pub stop_option: &'a StopOptions,
    pub departing_times: &'a FlightTimes,
    pub return_times: &'a FlightTimes,
    pub stopover_max: &'a StopoverDuration,
    /// Minimum layover duration (position \[11\] in the per-leg array).
    /// Defaults to [`StopoverDuration::UNLIMITED`] (no minimum imposed).
    pub stopover_min: &'a StopoverDuration,
    pub duration_max: &'a TotalDuration,
    pub frontend_version: &'a String,
    pub fixed_flights: &'a FixedFlights,
    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`, `"de"`.
    pub language: &'a str,
    /// ISO 3166-1 alpha-2 country code (upper-case), e.g. `"GB"`, `"FR"`.
    pub country: &'a str,
    /// Result sort order sent to Google Flights.
    pub sort_order: &'a SortOrder,
    /// Airlines / alliances to include (per-leg array position \[4\]).
    /// Empty = no restriction.
    pub airlines_include: &'a [AirlineFilter],
    /// Airlines / alliances to exclude (per-leg array position \[5\]).
    /// Empty = no restriction.
    pub airlines_exclude: &'a [AirlineFilter],
    /// Require a connection through these IATA airport codes (position \[9\]).
    /// Empty = no restriction.
    pub connecting_airports: &'a [String],
    /// If `true`, send `[1]` at position \[13\] to restrict to lower-CO₂ flights.
    pub lower_emissions: bool,
}

impl ToRequestBody for FlightRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&FlightRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;
    fn try_from(options: &FlightRequestOptions) -> Result<Self> {
        // Build the per-leg location vecs as closures so each leg gets its own
        // allocation without cloning a shared Vec.
        let make_dep = || vec![options.departing_city.iter().collect::<Vec<_>>()];
        let make_arr = || vec![options.arriving_city.iter().collect::<Vec<_>>()];

        let itinerary_going = options.fixed_flights.maybe_get_nth_flight_info(0_usize);
        let itinerary_return = options.fixed_flights.maybe_get_nth_flight_info(1_usize);

        let mut legs = vec![SingleLegStruct {
            departure: make_dep(),
            arrival: make_arr(),
            stop_options: options.stop_option,
            date: options.date_start,
            times: options.departing_times,
            stopover_max: options.stopover_max,
            stopover_min: options.stopover_min,
            duration_max: options.duration_max,
            chosen_itinerary: itinerary_going.as_ref(),
            airlines_include: options.airlines_include,
            airlines_exclude: options.airlines_exclude,
            connecting_airports: options.connecting_airports,
            lower_emissions: options.lower_emissions,
        }];
        if let Some(date_return) = options.date_return {
            legs.push(SingleLegStruct {
                // Outbound and return swap departure/arrival.
                departure: make_arr(),
                arrival: make_dep(),
                stop_options: options.stop_option,
                date: date_return,
                times: options.return_times,
                stopover_max: options.stopover_max,
                stopover_min: options.stopover_min,
                duration_max: options.duration_max,
                chosen_itinerary: itinerary_return.as_ref(),
                airlines_include: options.airlines_include,
                airlines_exclude: options.airlines_exclude,
                connecting_airports: options.connecting_airports,
                lower_emissions: options.lower_emissions,
            });
        }

        let is_booking = options.fixed_flights.is_full();

        let itinerary = ItineraryRequest {
            legs,
            travel_class: options.travel_class,
            travelers: &options.travellers,
            is_graph: false,
            sort_order: *options.sort_order,
        };

        // logic: If return is defined, choose token of return, else choose the one of the way there, else none.
        let departure_token = options.fixed_flights.get_departure_token();

        let complete_flight_request = CompleteFlightRequest {
            itinerary,
            departure_token: departure_token.as_deref(),
            is_booking,
        };
        let body = complete_flight_request.serialize_to_web()?;
        let endpoint = if is_booking {
            BOOKING_REQUEST
        } else {
            FLIGHT_REQUEST
        };
        let url = format!(
            "{endpoint}?f.sid=6921237406276106431&bl={}&hl={}-{}&soc-app=162&soc-platform=1&soc-device=1&_reqid=4150414&rt=c",
            options.frontend_version,
            options.language,
            options.country.to_uppercase()
        );
        let encoded = utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string();
        Ok(Self { url, body: encoded })
    }
}

#[derive(Debug, Clone)]
pub struct SingleLegStruct<'a> {
    pub departure: Vec<Vec<&'a Location>>,
    pub arrival: Vec<Vec<&'a Location>>,
    pub stop_options: &'a StopOptions,
    pub date: &'a str,
    pub times: &'a FlightTimes,
    pub stopover_max: &'a StopoverDuration,
    /// Minimum layover / connection duration.
    /// Serialized to position **\[11\]** of the per-leg array.
    /// Set to [`StopoverDuration::UNLIMITED`] (default) to impose no minimum.
    pub stopover_min: &'a StopoverDuration,
    pub duration_max: &'a TotalDuration,
    pub chosen_itinerary: Option<&'a Vec<FlightInfo>>,
    /// Airlines / alliances to include (position \[4\]).
    pub airlines_include: &'a [AirlineFilter],
    /// Airlines / alliances to exclude (position \[5\]).
    pub airlines_exclude: &'a [AirlineFilter],
    /// Connecting airport IATA codes (position \[9\]).
    pub connecting_airports: &'a [String],
    /// Lower-emissions filter (position \[13\]): sends `[1]` when `true`.
    pub lower_emissions: bool,
}

/// Serialise a slice of [`AirlineFilter`] values to the Google Flights wire
/// format: a JSON array of quoted strings, or `null` when empty.
///
/// Example: `["LX","LH"]` or `["ONEWORLD"]`
fn serialize_airline_filters(v: &[AirlineFilter]) -> String {
    if v.is_empty() {
        return "null".to_owned();
    }
    format!(
        "[{}]",
        v.iter()
            .map(|f| format!("\\\"{}\\\"", f.as_google_str()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Serialise a slice of IATA airport codes to the Google Flights wire format.
///
/// Example: `["CDG"]` or `null` when empty.
fn serialize_airport_list(v: &[String]) -> String {
    if v.is_empty() {
        return "null".to_owned();
    }
    format!(
        "[{}]",
        v.iter()
            .map(|s| format!("\\\"{}\\\"", s))
            .collect::<Vec<_>>()
            .join(",")
    )
}

impl SerializeToWeb for SingleLegStruct<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        // Per-leg array (15 elements, indices 0–14):
        //   [0]  departure airports
        //   [1]  arrival airports
        //   [2]  time-of-day filter [dep_min, dep_max, arr_min, arr_max] or null
        //   [3]  stops option (0=any, 1=nonstop, 2=≤1, 3=≤2)
        //   [4]  airline/alliance include filter  ["LX"] / ["ONEWORLD"] / null
        //   [5]  airline/alliance exclude filter  (mirror of [4])       / null
        //   [6]  departure date string "YYYY-MM-DD"
        //   [7]  max total duration [minutes] or null
        //   [8]  pre-selected itinerary (offer flow) or null
        //   [9]  connecting airport IATA codes ["CDG"] or null
        //   [10] unknown (always null)
        //   [11] min layover minutes or null
        //   [12] max layover minutes or null
        //   [13] lower-emissions flag [1] or null
        //   [14] display classifier: 3 = outbound / one-way
        let flight_to_show: i32 = 3;

        let chosen_itinerary = match self.chosen_itinerary {
            Some(x) => x.clone().serialize_to_web()?,
            None => "null".to_string(),
        };

        Ok(format!(
            r#"[{0},{1},{2},{3},{4},{5},\"{6}\",{7},{8},{9},null,{10},{11},{12},{13}]"#,
            &self.departure.serialize_to_web()?,    // [0]
            &self.arrival.serialize_to_web()?,      // [1]
            &self.times.serialize_to_web()?,        // [2]
            &self.stop_options.serialize_to_web()?, // [3]
            serialize_airline_filters(self.airlines_include), // [4]
            serialize_airline_filters(self.airlines_exclude), // [5]
            self.date,                              // [6]
            self.duration_max.serialize_to_web()?,  // [7]
            chosen_itinerary,                       // [8]
            serialize_airport_list(self.connecting_airports), // [9]
            // [10] hardcoded null (in format string above)
            self.stopover_min.serialize_to_web()?, // [11] ← FIXED (was [13])
            self.stopover_max.serialize_to_web()?, // [12]
            if self.lower_emissions { "[1]" } else { "null" }, // [13]
            flight_to_show,                        // [14]
        ))
    }
}

pub struct ItineraryRequest<'a> {
    // [null,null,{sort},null,[],{travel_class},[{travelers}],null,null,null,null,null,null,
    // [
    // [[[[\"{0}\",0]]],[[[\"{1}\",0]]],null,{8},null,null,\"{2}\",null,null,null,null,null,null,null,3]
    // ]
    // ,null,null,null,1]
    pub legs: Vec<SingleLegStruct<'a>>,
    pub travel_class: &'a TravelClass,
    pub travelers: &'a Travelers,
    pub is_graph: bool,
    /// Sort order sent to Google Flights (position 2 in the outer request array).
    pub sort_order: SortOrder,
}

impl<'a> ItineraryRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departure: &'a [Location],
        arrival: &'a [Location],
        stop_options: &'a StopOptions,
        date_start: &'a str,
        date_return: &'a Option<&str>,
        travelers: &'a Travelers,
        travel_class: &'a TravelClass,
        departing_times: &'a FlightTimes,
        return_times: &'a FlightTimes,
        stopover_max: &'a StopoverDuration,
        stopover_min: &'a StopoverDuration,
        duration_max: &'a TotalDuration,
        is_graph: bool,
        sort_order: SortOrder,
    ) -> Self {
        let mut legs = vec![];
        let first = SingleLegStruct {
            departure: vec![departure.iter().collect()],
            arrival: vec![arrival.iter().collect()],
            stop_options,
            date: date_start,
            times: departing_times,
            stopover_max,
            stopover_min,
            duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        legs.push(first);
        if let Some(x) = date_return {
            legs.push(SingleLegStruct {
                departure: vec![arrival.iter().collect()],
                arrival: vec![departure.iter().collect()],
                date: x,
                stop_options,
                times: return_times,
                stopover_max,
                stopover_min,
                duration_max,
                chosen_itinerary: None,
                airlines_include: &[],
                airlines_exclude: &[],
                connecting_airports: &[],
                lower_emissions: false,
            })
        };
        ItineraryRequest {
            legs,
            travel_class,
            travelers,
            is_graph,
            sort_order,
        }
    }
}

impl SerializeToWeb for ItineraryRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let graph = if self.is_graph { ",1" } else { "" };
        Ok(format!(
            // [null,null,{sort},null,[],{travel_class},{travelers},null×6,{legs},null×3,1{,1 graph}]
            r#"[null,null,{0},null,[],{1},{2},null,null,null,null,null,null,{3},null,null,null,1{4}]"#,
            self.sort_order as i32,
            &self.travel_class.serialize_to_web()?,
            &self.travelers.serialize_to_web()?,
            &self.legs.serialize_to_web()?,
            graph
        ))
    }
}

struct CompleteFlightRequest<'a> {
    itinerary: ItineraryRequest<'a>,
    departure_token: Option<&'a str>,
    /// `true` when all flight legs are fixed — triggers the `GetBookingResults`
    /// endpoint and the matching `null,0` body tail the browser uses.
    is_booking: bool,
}

impl SerializeToWeb for CompleteFlightRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let departure_token = match self.departure_token {
            Some(token) => format!(r#"[null,\"{}\"]"#, token),
            None => "[]".to_string(),
        };

        // Booking requests (GetBookingResults) use `null,0` as the body tail —
        // matching what the browser sends.  Regular shopping requests use `1,0,0`.
        let end_part = if self.is_booking { "null,0" } else { "1,0,0" };

        Ok(format!(
            r#"f.req=[null,"[{},{},{}]"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:{}&"#,
            departure_token,
            &self.itinerary.serialize_to_web()?,
            end_part,
            epoch_now
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::vec;

    use anyhow::Ok;
    use chrono::{Duration, Utc};

    use crate::parsers::common::PlaceType;
    use crate::parsers::response::flight_response::{AirplaneInfo, Date, Hour};

    fn future_date(days: i64) -> String {
        (Utc::now().date_naive() + Duration::days(days)).to_string()
    }

    use super::*;

    #[test]
    fn test_produce_correct_body() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0])?;
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
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(1_usize);
        let search_settings = FlightRequestOptions {
            departing_city: core::slice::from_ref(&departure),
            arriving_city: core::slice::from_ref(&arrival),
            date_start: "2024-02-02",
            date_return: None,
            travellers,
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &flight_times,
            return_times: &flight_times,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            frontend_version: &frontend_version,
            fixed_flights: &fixed_flights,
            language: "en",
            country: "GB",
            sort_order: &SortOrder::Best,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };

        let req: RequestBody = (&search_settings).try_into()?;
        let expected = "f.req=%5Bnull%2C%22%5B%5B%5D%2C%5Bnull%2Cnull%2C1%2Cnull%2C%5B%5D%2C1%2C%5B1%2C0%2C0%2C0%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-02-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%5D%2Cnull%2Cnull%2Cnull%2C1%5D%2C1%2C0%2C0%5D%22%5D&";
        assert!(req.body.starts_with(expected));

        assert!(req.url.contains(&frontend_version));
        Ok(())
    }

    #[test]
    fn test_produce_correct_body_return() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0])?;
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
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(2_usize);
        let search_settings = FlightRequestOptions {
            departing_city: core::slice::from_ref(&departure),
            arriving_city: core::slice::from_ref(&arrival),
            date_start: "2024-02-02",
            date_return: Some("2024-03-02"),
            travellers,
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &flight_times,
            return_times: &flight_times,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            frontend_version: &frontend_version,
            fixed_flights: &fixed_flights,
            language: "en",
            country: "GB",
            sort_order: &SortOrder::Best,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };

        let req: RequestBody = (&search_settings).try_into()?;
        let expected = "f.req=%5Bnull%2C%22%5B%5B%5D%2C%5Bnull%2Cnull%2C1%2Cnull%2C%5B%5D%2C1%2C%5B1%2C0%2C0%2C0%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-02-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%2C%5B%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-03-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%5D%2Cnull%2Cnull%2Cnull%2C1%5D%2C1%2C0%2C0%5D%22%5D";
        assert!(req.body.starts_with(expected));
        Ok(())
    }

    #[test]
    fn test_result() -> Result<()> {
        let a = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        assert_eq!(a.serialize_to_web()?, r#"[\"MXP\",0]"#);
        Ok(())
    }

    #[test]
    fn test_result_comp() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_result_filter_departure() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let binding = FlightTimes::new(8, 23, 0, 23);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],[8,23,0,23],0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_stopover_duration() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::Minutes(250);
        let duration_max = TotalDuration::Minutes(600);
        let binding = FlightTimes::default();
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",[600],null,null,null,null,270,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_result_filter_arrival() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::new(0, 23, 8, 11);
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],[0,23,8,11],0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_serialize_itinerary_request() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let binding = FlightTimes::default();
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let first = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-10-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let second = SingleLegStruct {
            departure: vec![vec![&arrival]],
            arrival: vec![vec![&departure]],
            stop_options: &StopOptions::All,
            date: "2022-10-30",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let travelers = Travelers::new([1, 0, 0, 0].to_vec())?;

        let itinerary = ItineraryRequest {
            legs: vec![first.clone()],
            travel_class: &TravelClass::Economy,
            travelers: &travelers,
            is_graph: false,
            sort_order: SortOrder::Best,
        };

        let expected_single_leg = r#"[null,null,1,null,[],1,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3]],null,null,null,1]"#;
        assert_eq!(itinerary.serialize_to_web()?, expected_single_leg);
        let expected_two_legs = r#"[null,null,1,null,[],1,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3],[[[[\"CDG\",0]]],[[[\"MXP\",0]]],null,0,null,null,\"2022-10-30\",null,null,null,null,null,null,null,3]],null,null,null,1]"#;
        let itinerary_return = ItineraryRequest {
            legs: vec![first, second],
            travel_class: &TravelClass::Economy,
            travelers: &travelers,
            is_graph: false,
            sort_order: SortOrder::Best,
        };
        assert_eq!(itinerary_return.serialize_to_web()?, expected_two_legs);
        Ok(())
    }

    #[test]
    fn test_complete_flight_request() -> Result<()> {
        let travelers = Travelers::new([1, 0, 0, 0].to_vec())?;

        let expected_two_legs = r#"f.req=[null,"[[],[null,null,1,null,[],4,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3],[[[[\"CDG\",0]]],[[[\"MXP\",0]]],null,0,null,null,\"2022-10-30\",null,null,null,null,null,null,null,3]],null,null,null,1],1,0,0]"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:"#;

        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let itinerary_return = ItineraryRequest::new(
            core::slice::from_ref(&departure),
            core::slice::from_ref(&arrival),
            &StopOptions::All,
            "2022-10-20",
            &Some("2022-10-30"),
            &travelers,
            &TravelClass::First,
            &binding,
            &binding,
            &stopover_max,
            &StopoverDuration::UNLIMITED,
            &duration_max,
            false,
            SortOrder::Best,
        );

        let complete_req = CompleteFlightRequest {
            itinerary: itinerary_return,
            departure_token: None,
            is_booking: false,
        };
        assert!(complete_req
            .serialize_to_web()?
            .starts_with(expected_two_legs));
        Ok(())
    }

    #[test]
    fn test_with_choosen_leg() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: Some(&choosen_itinerary),
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_with_chosen_leg_stopover_airports() -> Result<()> {
        let departure = Location {
            loc_identifier: "/m/0947l".to_owned(),
            loc_type: PlaceType::City,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "/m/05qtj".to_owned(),
            loc_type: PlaceType::City,
            location_name: None,
        };
        let stopover_max = StopoverDuration::Minutes(420_u32);
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: Some(&choosen_itinerary),
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"/m/0947l\",5]]],[[[\"/m/05qtj\",5]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,420,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_with_chosen_leg_stopover_cities() -> Result<()> {
        let departure = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::Minutes(420_u32);
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::All,
            date: "2022-11-20",
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: Some(&choosen_itinerary),
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,420,null,3]"#
        );
        Ok(())
    }

    /// Two departure airports in one group serialise as two entries inside the
    /// innermost array: `[[["LHR",0],["LGW",0]]]`.
    #[test]
    fn test_multi_airport_departure_serializes_correctly() -> Result<()> {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let lgw = Location {
            loc_identifier: "LGW".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let date = future_date(30);

        let leg = SingleLegStruct {
            departure: vec![vec![&lhr, &lgw]],
            arrival: vec![vec![&jfk]],
            stop_options: &StopOptions::All,
            date: &date,
            times: &binding,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let expected = format!(
            r#"[[[[\"LHR\",0],[\"LGW\",0]]],[[[\"JFK\",0]]],null,0,null,null,\"{date}\",null,null,null,null,null,null,null,3]"#
        );
        assert_eq!(leg.serialize_to_web()?, expected);
        Ok(())
    }

    /// `FlightRequestOptions::new` with multiple departure airports produces a
    /// request body that contains all airport codes.
    #[test]
    fn test_flight_request_options_multi_departure() -> Result<()> {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let lgw = Location {
            loc_identifier: "LGW".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let departures = [lhr, lgw];
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let flight_times = FlightTimes::default();
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(1_usize);
        let date = future_date(30);

        let opts = FlightRequestOptions {
            departing_city: &departures,
            arriving_city: core::slice::from_ref(&jfk),
            date_start: &date,
            date_return: None,
            travellers: Travelers::new(vec![1, 0, 0, 0]).expect("valid traveler counts"),
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &flight_times,
            return_times: &flight_times,
            stopover_max: &stopover_max,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &duration_max,
            frontend_version: &frontend_version,
            fixed_flights: &fixed_flights,
            language: "en",
            country: "GB",
            sort_order: &SortOrder::Best,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let req: RequestBody = (&opts).try_into()?;
        // Both LHR and LGW must appear in the body; JFK as the single arrival.
        assert!(req.body.contains("LHR"), "body should contain LHR");
        assert!(req.body.contains("LGW"), "body should contain LGW");
        assert!(req.body.contains("JFK"), "body should contain JFK");
        Ok(())
    }

    fn generate_itinerary_data() -> Vec<FlightInfo> {
        let choosen_itinerary_1 = FlightInfo {
            departure_airport_code: "MXP".to_owned(),
            destination_airport_code: "LHR".to_owned(),
            departure_time: Hour {
                hour: Some(10),
                minute: 0,
            },
            arrival_time: Hour {
                hour: Some(12),
                minute: 0,
            },
            leg_duration_minutes: None,
            departure_date: Date {
                year: 2024,
                month: 2,
                day: 1,
            },
            arrival_date: Date {
                year: 2024,
                month: 2,
                day: 1,
            },
            airplane_info: AirplaneInfo {
                code: "BA".to_string(),
                flight_number: "420".to_owned(),
                plane_crew_by: None,
                name: "777".to_string(),
            },
        };
        let choosen_itinerary_2 = FlightInfo {
            departure_airport_code: "LHR".to_owned(),
            destination_airport_code: "CDG".to_owned(),
            departure_time: Hour {
                hour: Some(13),
                minute: 0,
            },
            arrival_time: Hour {
                hour: Some(14),
                minute: 0,
            },
            leg_duration_minutes: None,
            departure_date: Date {
                year: 2024,
                month: 2,
                day: 1,
            },
            arrival_date: Date {
                year: 2024,
                month: 2,
                day: 1,
            },
            airplane_info: AirplaneInfo {
                code: "AF".to_string(),
                flight_number: "350".to_owned(),
                plane_crew_by: None,
                name: "777".to_string(),
            },
        };

        [choosen_itinerary_1, choosen_itinerary_2].to_vec()
    }

    // -----------------------------------------------------------------------
    // serialize_airline_filters / serialize_airport_list
    // -----------------------------------------------------------------------

    #[test]
    fn serialize_airline_filters_empty_returns_null() {
        assert_eq!(serialize_airline_filters(&[]), "null");
    }

    #[test]
    fn serialize_airline_filters_single_iata() {
        use crate::parsers::common::AirlineCode;
        let f = AirlineFilter::Airline(AirlineCode::new("LX").unwrap());
        assert_eq!(serialize_airline_filters(&[f]), r#"[\"LX\"]"#);
    }

    #[test]
    fn serialize_airline_filters_alliance() {
        use crate::parsers::common::Alliance;
        let f = AirlineFilter::Alliance(Alliance::OneWorld);
        assert_eq!(serialize_airline_filters(&[f]), r#"[\"ONEWORLD\"]"#);
    }

    #[test]
    fn serialize_airline_filters_mixed_multiple() {
        use crate::parsers::common::{AirlineCode, Alliance};
        let filters = vec![
            AirlineFilter::Airline(AirlineCode::new("LH").unwrap()),
            AirlineFilter::Alliance(Alliance::SkyTeam),
        ];
        assert_eq!(
            serialize_airline_filters(&filters),
            r#"[\"LH\",\"SKYTEAM\"]"#
        );
    }

    #[test]
    fn serialize_airport_list_empty_returns_null() {
        assert_eq!(serialize_airport_list(&[]), "null");
    }

    #[test]
    fn serialize_airport_list_single() {
        assert_eq!(serialize_airport_list(&["CDG".to_string()]), r#"[\"CDG\"]"#);
    }

    #[test]
    fn serialize_airport_list_multiple() {
        assert_eq!(
            serialize_airport_list(&["CDG".to_string(), "AMS".to_string()]),
            r#"[\"CDG\",\"AMS\"]"#
        );
    }

    /// Verify that lower_emissions=true produces `[1]` at position [13]
    /// and false produces `null`.
    #[test]
    fn single_leg_lower_emissions_serialization() {
        let dep = Location {
            loc_identifier: "LHR".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arr = Location {
            loc_identifier: "JFK".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let times = FlightTimes::default();
        let leg_no_emissions = SingleLegStruct {
            departure: vec![vec![&dep]],
            arrival: vec![vec![&arr]],
            times: &times,
            stop_options: &StopOptions::All,
            date: "2026-08-01",
            stopover_max: &StopoverDuration::UNLIMITED,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &TotalDuration::UNLIMITED,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let without = leg_no_emissions.serialize_to_web().unwrap();
        assert!(without.ends_with(",null,3]"), "no emissions: {without}");

        let leg_with_emissions = SingleLegStruct {
            lower_emissions: true,
            ..leg_no_emissions
        };
        let with_str = leg_with_emissions.serialize_to_web().unwrap();
        assert!(with_str.ends_with(",[1],3]"), "with emissions: {with_str}");
    }

    // -----------------------------------------------------------------------
    // Wire protocol: airline-include filter appears in the request body
    // -----------------------------------------------------------------------

    /// When `airlines_include` is set, the filter string (e.g. `LX`) must
    /// appear in the serialised `f.req` body.
    #[test]
    fn request_body_contains_airline_include_filter() -> Result<()> {
        use crate::parsers::common::AirlineCode;

        let lx = AirlineFilter::Airline(AirlineCode::new("LX").unwrap());
        let departure = Location {
            loc_identifier: "LUX".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "NRT".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(1);
        let times = FlightTimes::default();

        let opts = FlightRequestOptions {
            departing_city: core::slice::from_ref(&departure),
            arriving_city: core::slice::from_ref(&arrival),
            date_start: "2025-06-01",
            date_return: None,
            travellers: Travelers::new(vec![1, 0, 0, 0])?,
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &times,
            return_times: &times,
            stopover_max: &StopoverDuration::UNLIMITED,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &TotalDuration::UNLIMITED,
            frontend_version: &frontend_version,
            fixed_flights: &fixed_flights,
            language: "en",
            country: "GB",
            sort_order: &SortOrder::Best,
            airlines_include: &[lx],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let req: RequestBody = (&opts).try_into()?;
        assert!(req.body.contains("LX"), "body should contain airline code LX");
        Ok(())
    }

    /// When `stop_option` is `NonStop`, the stops value in the body must be `1`.
    #[test]
    fn request_body_nonstop_filter_sets_stop_value_1() -> Result<()> {
        let departure = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let times = FlightTimes::default();
        let leg = SingleLegStruct {
            departure: vec![vec![&departure]],
            arrival: vec![vec![&arrival]],
            stop_options: &StopOptions::NoStop,
            date: "2025-06-01",
            times: &times,
            stopover_max: &StopoverDuration::UNLIMITED,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &TotalDuration::UNLIMITED,
            chosen_itinerary: None,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        let serialised = leg.serialize_to_web()?;
        // position [3] is the stop option; NonStop → 1
        assert!(
            serialised.contains(",1,null,null,"),
            "nonstop leg should contain ',1,null,null,' at stop-option position: {serialised}"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Locale / language parameter tests
    // -----------------------------------------------------------------------

    /// `language="fr"` + `country="FR"` must produce `hl=fr-FR` in the URL.
    #[test]
    fn request_url_contains_hl_fr_fr() -> Result<()> {
        let req = make_minimal_request("fr", "FR")?;
        assert!(
            req.url.contains("hl=fr-FR"),
            "URL should contain hl=fr-FR, got: {}",
            req.url
        );
        Ok(())
    }

    /// `language="de"` + `country="DE"` must produce `hl=de-DE` in the URL.
    #[test]
    fn request_url_contains_hl_de_de() -> Result<()> {
        let req = make_minimal_request("de", "DE")?;
        assert!(
            req.url.contains("hl=de-DE"),
            "URL should contain hl=de-DE, got: {}",
            req.url
        );
        Ok(())
    }

    /// `country` stored in lowercase is uppercased in the URL.
    #[test]
    fn request_url_uppercases_country_code() -> Result<()> {
        let req = make_minimal_request("en", "gb")?;
        assert!(
            req.url.contains("hl=en-GB"),
            "country 'gb' should be uppercased to 'GB' in URL: {}",
            req.url
        );
        Ok(())
    }

    /// Helper: build a minimal `RequestBody` with the given locale params.
    fn make_minimal_request(language: &str, country: &str) -> Result<RequestBody> {
        let departure = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let arrival = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let times = FlightTimes::default();
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(1);

        let opts = FlightRequestOptions {
            departing_city: core::slice::from_ref(&departure),
            arriving_city: core::slice::from_ref(&arrival),
            date_start: "2025-06-01",
            date_return: None,
            travellers: Travelers::new(vec![1, 0, 0, 0])?,
            travel_class: &TravelClass::Economy,
            stop_option: &StopOptions::All,
            departing_times: &times,
            return_times: &times,
            stopover_max: &StopoverDuration::UNLIMITED,
            stopover_min: &StopoverDuration::UNLIMITED,
            duration_max: &TotalDuration::UNLIMITED,
            frontend_version: &frontend_version,
            fixed_flights: &fixed_flights,
            language,
            country,
            sort_order: &SortOrder::Best,
            airlines_include: &[],
            airlines_exclude: &[],
            connecting_airports: &[],
            lower_emissions: false,
        };
        (&opts).try_into()
    }
}
