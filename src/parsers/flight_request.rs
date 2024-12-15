use std::{
    time::{SystemTime, UNIX_EPOCH},
    vec,
};

use percent_encoding::utf8_percent_encode;

use crate::{
    parsers::common::{FixedFlights, FlightTimes, StopoverDuration, TotalDuration},
    parsers::flight_response::FlightInfo,
};

use super::common::{
    Location, RequestBody, SerializeToWeb, StopOptions, ToRequestBody, TravelClass, Travelers,
    CHARACTERS_TO_ENCODE,
};

use anyhow::Result;

pub struct FlightRequestOptions<'a> {
    deparing_city: &'a Location,
    arriving_city: &'a Location,
    date_start: &'a str,
    date_return: Option<&'a str>,
    travellers: Travelers,
    travel_class: &'a TravelClass,
    stop_option: &'a StopOptions,
    departing_times: &'a FlightTimes,
    return_times: &'a FlightTimes,
    stopover_max: &'a StopoverDuration,
    duration_max: &'a TotalDuration,
    frontend_version: &'a String,
    fixed_flights: &'a FixedFlights,
}

impl<'a> FlightRequestOptions<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departing_city: &'a Location,
        arriving_city: &'a Location,
        date_start: &'a str,
        date_return: Option<&'a str>,
        travellers: Travelers,
        travel_class: &'a TravelClass,
        stop_option: &'a StopOptions,
        departing_times: &'a FlightTimes,
        return_times: &'a FlightTimes,
        stopover_max: &'a StopoverDuration,
        duration_max: &'a TotalDuration,
        frontend_version: &'a String,
        fixed_flights: &'a FixedFlights,
    ) -> Self {
        Self {
            deparing_city: departing_city,
            arriving_city,
            date_start,
            date_return,
            travellers,
            travel_class,
            stop_option,
            departing_times,
            return_times,
            stopover_max,
            duration_max,
            frontend_version,
            fixed_flights,
        }
    }
}

impl ToRequestBody for FlightRequestOptions<'_> {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&FlightRequestOptions<'_>> for RequestBody {
    type Error = anyhow::Error;
    fn try_from(options: &FlightRequestOptions) -> Result<Self> {
        let departure = vec![vec![options.deparing_city]];
        let arrival = vec![vec![options.arriving_city]];
        let itinerary_going = options.fixed_flights.maybe_get_nth_flight_info(0_usize);
        let itinerary_return = options.fixed_flights.maybe_get_nth_flight_info(1_usize);
        let leg1 = SingleLegStruct {
            departure: departure.clone(),
            arrival: arrival.clone(),
            stop_options: options.stop_option,
            date: options.date_start,
            times: options.departing_times,
            stopover_max: options.stopover_max,
            duration_max: options.duration_max,
            chosen_itinerary: itinerary_going.as_ref(),
        };
        let leg2 = options.date_return.map(|date_return| SingleLegStruct {
            departure: arrival,
            arrival: departure,
            stop_options: options.stop_option,
            date: date_return,
            times: options.return_times,
            stopover_max: options.stopover_max,
            duration_max: options.duration_max,
            chosen_itinerary: itinerary_return.as_ref(),
        });
        let legs: Vec<SingleLegStruct<'_>> = if let Some(leg_2) = leg2 {
            vec![leg1, leg_2]
        } else {
            vec![leg1]
        };

        let itinerary = ItineraryRequest {
            legs,
            travel_class: options.travel_class,
            travelers: &options.travellers,
            is_graph: false,
        };

        // logic: If return is defined, choose token of return, else choose the one of the way there, else none.
        let departure_token = options.fixed_flights.get_departure_token();

        let complete_flight_request = CompleteFlightRequest {
            itinerary,
            departure_token: departure_token.as_deref(),
            is_second_leg: options.fixed_flights.is_full(),
        };
        let body = complete_flight_request.serialize_to_web()?;
        let url = match options.fixed_flights.is_full() {
            true => format!("https://www.google.com/_/TravelFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetBookingResults?f.sid=6921237406276106431&bl={}&hl=en-GB&soc-app=162&soc-platform=1&soc-device=1&_reqid=4150414&rt=c", options.frontend_version),
            false =>   format!("https://www.google.com/_/TravelFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetShoppingResults?f.sid=6921237406276106431&bl={}&hl=en-GB&soc-app=162&soc-platform=1&soc-device=1&_reqid=4150414&rt=c", options.frontend_version)
        };
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
    times: &'a FlightTimes,
    stopover_max: &'a StopoverDuration,
    duration_max: &'a TotalDuration,
    chosen_itinerary: Option<&'a Vec<FlightInfo>>,
}

impl<'a> SingleLegStruct<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departure: Vec<Vec<&'a Location>>,
        arrival: Vec<Vec<&'a Location>>,
        stop_options: &'a StopOptions,
        date: &'a str,
        times: &'a FlightTimes,
        stopover_max: &'a StopoverDuration,
        duration_max: &'a TotalDuration,
        chosen_itinerary: Option<&'a Vec<FlightInfo>>,
    ) -> Self {
        Self {
            departure,
            arrival,
            stop_options,
            date,
            times,
            stopover_max,
            duration_max,
            chosen_itinerary,
        }
    }
}

impl SerializeToWeb for SingleLegStruct<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        // [[[[\\"/m/0fq8f\\",4]]],[[[\\"/m/0947l\\",5]]],[0,8,0,23],0,null,null,\\"2024-02-06\\",null,null,null,null,null,null,null,3]
        //TODO magic number
        let flight_to_show = 3; //All flights. 1= only some

        let chosen_itinerary = match self.chosen_itinerary {
            Some(x) => x.clone().serialize_to_web()?,
            None => "null".to_string(),
        };
        Ok(format!(
            r#"[{0},{1},{2},{3},null,null,\"{4}\",{5},{6},null,null,null,{7},null,{8}]"#,
            &self.departure.serialize_to_web()?,
            &self.arrival.serialize_to_web()?,
            &self.times.serialize_to_web()?,
            &self.stop_options.serialize_to_web()?,
            self.date,
            self.duration_max.serialize_to_web()?,
            chosen_itinerary,
            self.stopover_max.serialize_to_web()?,
            flight_to_show,
        ))
    }
}

pub struct ItineraryRequest<'a> {
    // [null,null,2,null,[],{7},[{3},{4},{5},{6}],null,null,null,null,null,null,
    // [
    // [[[[\"{0}\",0]]],[[[\"{1}\",0]]],null,{8},null,null,\"{2}\",null,null,null,null,null,null,null,3]
    // ]
    // ,null,null,null,{7}]
    pub legs: Vec<SingleLegStruct<'a>>,
    pub travel_class: &'a TravelClass,
    pub travelers: &'a Travelers,
    pub is_graph: bool,
}

impl<'a> ItineraryRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departure: &'a Location,
        arrival: &'a Location,
        stop_options: &'a StopOptions,
        date_start: &'a str,
        date_return: &'a Option<&str>,
        travelers: &'a Travelers,
        travel_class: &'a TravelClass,
        departing_times: &'a FlightTimes,
        return_times: &'a FlightTimes,
        stopover_max: &'a StopoverDuration,
        duration_max: &'a TotalDuration,
        is_graph: bool,
    ) -> Self {
        let mut legs = vec![];
        let first = SingleLegStruct {
            departure: vec![vec![departure]],
            arrival: vec![vec![arrival]],
            stop_options,
            date: date_start,
            times: departing_times,
            stopover_max,
            duration_max,
            chosen_itinerary: None,
        };
        legs.push(first);
        if let Some(x) = date_return {
            legs.push(SingleLegStruct {
                departure: vec![vec![arrival]],
                arrival: vec![vec![departure]],
                date: x,
                stop_options,
                times: return_times,
                stopover_max,
                duration_max,
                chosen_itinerary: None,
            })
        };
        ItineraryRequest {
            legs,
            travel_class,
            travelers,
            is_graph,
        }
    }
}

impl SerializeToWeb for ItineraryRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let graph = if self.is_graph { ",1" } else { "" };
        // number seems to be 1 for requests after the first, else 2
        let number = match self.legs.first() {
            Some(x) => match x.chosen_itinerary {
                Some(_) => "1",
                None => "2",
            },
            None => "2",
        };
        Ok(format!(
            // travel,travelers,legs,graph
            // [null,null,2,null,[],{0},{1},null,null,null,null,null,null,{2},null,null,null,1,1] Graph request has 1 more 1 at the end
            r#"[null,null,{0},null,[],{1},{2},null,null,null,null,null,null,{3},null,null,null,1{4}]"#,
            number,
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
    is_second_leg: bool,
}

impl SerializeToWeb for CompleteFlightRequest<'_> {
    fn serialize_to_web(&self) -> Result<String> {
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let departure_token = match self.departure_token {
            Some(token) => format!(r#"[null,\"{}\"]"#, token),
            None => "[]".to_string(),
        };

        //TODOno difference for the moment.
        let end_part = match self.is_second_leg {
            false => "1,0,0".to_owned(),
            true => "null,0".to_owned(),
        };

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
mod tests {
    use std::vec;

    use anyhow::Ok;

    use crate::parsers::flight_response::{AirplaneInfo, Date, Hour};

    use super::*;

    #[test]
    fn test_produce_correct_body() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0]);
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("SYD", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let flight_times = FlightTimes::default();
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(1_usize);
        let search_settings: FlightRequestOptions = FlightRequestOptions::new(
            &departure,
            &arrival,
            "2024-02-02",
            None,
            travellers,
            &TravelClass::Economy,
            &StopOptions::All,
            &flight_times,
            &flight_times,
            &stopover_max,
            &duration_max,
            &frontend_version,
            &fixed_flights,
        );

        let req: RequestBody = (&search_settings).try_into()?;
        let expected = "f.req=%5Bnull%2C%22%5B%5B%5D%2C%5Bnull%2Cnull%2C2%2Cnull%2C%5B%5D%2C1%2C%5B1%2C0%2C0%2C0%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-02-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%5D%2Cnull%2Cnull%2Cnull%2C1%5D%2C1%2C0%2C0%5D%22%5D&";
        assert!(req.body.starts_with(expected));

        assert!(req.url.contains(&frontend_version));
        Ok(())
    }

    #[test]
    fn test_produce_correct_body_return() -> Result<()> {
        let travellers = Travelers::new(vec![1, 0, 0, 0]);
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("SYD", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let flight_times = FlightTimes::default();
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let fixed_flights = FixedFlights::new(2_usize);
        let search_settings: FlightRequestOptions = FlightRequestOptions::new(
            &departure,
            &arrival,
            "2024-02-02",
            Some("2024-03-02"),
            travellers,
            &TravelClass::Economy,
            &StopOptions::All,
            &flight_times,
            &flight_times,
            &stopover_max,
            &duration_max,
            &frontend_version,
            &fixed_flights,
        );

        let req: RequestBody = (&search_settings).try_into()?;
        let expected = "f.req=%5Bnull%2C%22%5B%5B%5D%2C%5Bnull%2Cnull%2C2%2Cnull%2C%5B%5D%2C1%2C%5B1%2C0%2C0%2C0%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-02-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%2C%5B%5B%5B%5B%5C%22SYD%5C%22%2C0%5D%5D%5D%2C%5B%5B%5B%5C%22MXP%5C%22%2C0%5D%5D%5D%2Cnull%2C0%2Cnull%2Cnull%2C%5C%222024-03-02%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C3%5D%5D%2Cnull%2Cnull%2Cnull%2C1%5D%2C1%2C0%2C0%5D%22%5D";
        assert!(req.body.starts_with(expected));
        Ok(())
    }

    #[test]
    fn test_result() -> Result<()> {
        let a = Location::new("MXP", 0, None);
        assert_eq!(a.serialize_to_web()?, r#"[\"MXP\",0]"#);
        Ok(())
    }

    #[test]
    fn test_result_comp() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_result_filter_departure() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let binding = FlightTimes::new(8, 23, 0, 23);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],[8,23,0,23],0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_stopover_duration() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::Minutes(250);
        let duration_max = TotalDuration::Minutes(600);
        let binding = FlightTimes::default();
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",[600],null,null,null,null,270,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_result_filter_arrival() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::new(0, 23, 8, 11);
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],[0,23,8,11],0,null,null,\"2022-11-20\",null,null,null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_serialize_itinerary_request() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let binding = FlightTimes::default();
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let first = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-10-20",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        let second = SingleLegStruct::new(
            vec![vec![&arrival]],
            vec![vec![&departure]],
            &StopOptions::All,
            "2022-10-30",
            &binding,
            &stopover_max,
            &duration_max,
            None,
        );
        let travelers = Travelers::new([1, 0, 0, 0].to_vec());

        let itinerary = ItineraryRequest {
            legs: vec![first.clone()],
            travel_class: &TravelClass::Economy,
            travelers: &travelers,
            is_graph: false,
        };

        let expected_single_leg = r#"[null,null,2,null,[],1,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3]],null,null,null,1]"#;
        assert_eq!(itinerary.serialize_to_web()?, expected_single_leg);
        let expected_two_legs = r#"[null,null,2,null,[],1,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3],[[[[\"CDG\",0]]],[[[\"MXP\",0]]],null,0,null,null,\"2022-10-30\",null,null,null,null,null,null,null,3]],null,null,null,1]"#;
        let itinerary_return = ItineraryRequest {
            legs: vec![first, second],
            travel_class: &TravelClass::Economy,
            travelers: &travelers,
            is_graph: false,
        };
        assert_eq!(itinerary_return.serialize_to_web()?, expected_two_legs);
        Ok(())
    }

    #[test]
    fn test_complete_flight_request() -> Result<()> {
        let travelers = Travelers::new([1, 0, 0, 0].to_vec());

        let expected_two_legs = r#"f.req=[null,"[[],[null,null,2,null,[],4,[1,0,0,0],null,null,null,null,null,null,[[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-10-20\",null,null,null,null,null,null,null,3],[[[[\"CDG\",0]]],[[[\"MXP\",0]]],null,0,null,null,\"2022-10-30\",null,null,null,null,null,null,null,3]],null,null,null,1],1,0,0]"]&at=AAuQa1qiXfSThbBOCdcDUAVTopoc:"#;

        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let itinerary_return = ItineraryRequest::new(
            &departure,
            &arrival,
            &StopOptions::All,
            "2022-10-20",
            &Some("2022-10-30"),
            &travelers,
            &TravelClass::First,
            &binding,
            &binding,
            &stopover_max,
            &duration_max,
            false,
        );

        let complete_req = CompleteFlightRequest {
            itinerary: itinerary_return,
            departure_token: None,
            is_second_leg: false,
        };
        assert!(complete_req
            .serialize_to_web()?
            .starts_with(expected_two_legs));
        Ok(())
    }

    #[test]
    fn test_with_choosen_leg() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::UNLIMITED;
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            Some(&choosen_itinerary),
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,null,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_with_chosen_leg_stopover_airports() -> Result<()> {
        let departure = Location::new("/m/0947l", 5, None);
        let arrival = Location::new("/m/05qtj", 5, None);
        let stopover_max = StopoverDuration::Minutes(420_u32);
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            Some(&choosen_itinerary),
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"/m/0947l\",5]]],[[[\"/m/05qtj\",5]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,420,null,3]"#
        );
        Ok(())
    }

    #[test]
    fn test_with_chosen_leg_stopover_cities() -> Result<()> {
        let departure = Location::new("MXP", 0, None);
        let arrival = Location::new("CDG", 0, None);
        let stopover_max = StopoverDuration::Minutes(420_u32);
        let duration_max = TotalDuration::UNLIMITED;
        let binding = FlightTimes::default();
        let choosen_itinerary = generate_itinerary_data();
        let a = SingleLegStruct::new(
            vec![vec![&departure]],
            vec![vec![&arrival]],
            &StopOptions::All,
            "2022-11-20",
            &binding,
            &stopover_max,
            &duration_max,
            Some(&choosen_itinerary),
        );
        assert_eq!(
            a.serialize_to_web()?,
            r#"[[[[\"MXP\",0]]],[[[\"CDG\",0]]],null,0,null,null,\"2022-11-20\",null,[[\"MXP\",\"2024-02-01\",\"LHR\",null,\"BA\",\"420\"],[\"LHR\",\"2024-02-01\",\"CDG\",null,\"AF\",\"350\"]],null,null,null,420,null,3]"#
        );
        Ok(())
    }

    fn generate_itinerary_data() -> Vec<FlightInfo> {
        let choosen_itinerary_1 = FlightInfo::new(
            "MXP".to_owned(),
            "LHR".to_owned(),
            Hour::new(Some(10), 0),
            Hour::new(Some(12), 0),
            Date::new(2024, 2, 1),
            Date::new(2024, 2, 1),
            AirplaneInfo::new("BA".to_string(), "420".to_owned(), None, "777".to_string()),
        );
        let choosen_itinerary_2 = FlightInfo::new(
            "LHR".to_owned(),
            "CDG".to_owned(),
            Hour::new(Some(13), 0),
            Hour::new(Some(14), 0),
            Date::new(2024, 2, 1),
            Date::new(2024, 2, 1),
            AirplaneInfo::new("AF".to_string(), "350".to_owned(), None, "777".to_string()),
        );

        [choosen_itinerary_1, choosen_itinerary_2].to_vec()
    }
}
