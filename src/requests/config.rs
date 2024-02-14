use core::panic;

use crate::protos::urls::Location as LocationProto;
use chrono::{Months, NaiveDate};
use parsers::common::{
    FlightTimes, Location, PlaceType, StopOptions, StopoverDuration, TotalDuration, TravelClass,
    Travelers,
};

use protos::urls::{ItineraryUrl, Leg};

#[derive(Debug, Clone)]
pub enum TripType {
    OneWay,
    Return,
    MultiCity,
}
impl Default for TripType {
    fn default() -> Self {
        Self::OneWay
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub departing_date: NaiveDate,
    pub departure: Location,
    pub destination: Location,
    pub stop_options: StopOptions,
    pub travel_class: TravelClass,
    pub return_date: Option<NaiveDate>,
    pub travellers: Travelers,
    pub departing_times: FlightTimes,
    pub return_times: FlightTimes,
    pub stopover_max: StopoverDuration,
    pub duration_max: TotalDuration,
    pub trip_type: TripType,
}

impl Config {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departing_date: NaiveDate,
        departure: Location,
        destination: Location,
        stop_options: StopOptions,
        travel_class: TravelClass,
        return_date: Option<NaiveDate>,
        travellers: Travelers,
        departing_times: FlightTimes,
        return_times: FlightTimes,
        stopover_max: StopoverDuration,
        duration_max: TotalDuration,
    ) -> Self {
        let trip_type = match return_date {
            Some(_) => TripType::Return,
            None => TripType::OneWay,
        };
        Self {
            departing_date,
            departure,
            destination,
            stop_options,
            travel_class,
            return_date,
            travellers,
            departing_times,
            return_times,
            stopover_max,
            duration_max,
            trip_type,
        }
    }

    pub fn get_diff_days(&self) -> Option<i64> {
        self.return_date
            .map(|x| x.signed_duration_since(self.departing_date).num_days())
    }

    pub fn get_end_graph(&self, months: Months) -> NaiveDate {
        self.departing_date.checked_add_months(months).unwrap()
    }
    pub fn to_flight_url(&self) -> String {
        ItineraryUrl::from(self).to_flight_url()
    }
    pub fn to_encoded(&self) -> String {
        ItineraryUrl::from(self).to_encoded()
    }
}

impl From<&Config> for Vec<Leg> {
    fn from(options: &Config) -> Vec<Leg> {
        let stops = match options.stop_options {
            StopOptions::All => None,
            variant => Some(variant as i32 - 1),
        };
        let max_stopover_minutes = options.stopover_max.to_option();
        let max_duration_minutes = options.duration_max.to_option();

        let departure = location_to_location_proto(&options.departure);

        let destination = location_to_location_proto(&options.destination);

        let first_leg = Leg {
            date: options.departing_date.to_string(),
            departure: vec![departure.clone()],
            arrival: vec![destination.clone()],
            min_hour_departure: options.departing_times.get_departure_hour_min(),
            max_hour_departure: options.departing_times.get_departure_hour_max(),
            min_hour_arrival: options.departing_times.get_arrival_hour_min(),
            max_hour_arrival: options.departing_times.get_arrival_hour_max(),
            stops,
            max_stopover_minutes,
            max_duration_minutes,
        };

        let mut leg_vector: Vec<Leg> = Vec::new();
        leg_vector.push(first_leg);

        match options.trip_type {
            TripType::OneWay => {} //already done
            TripType::Return => {
                let second_leg = Leg {
                    date: options.return_date.unwrap().to_string(),
                    departure: vec![destination.clone()],
                    arrival: vec![departure.clone()],
                    min_hour_departure: options.return_times.get_departure_hour_min(),
                    max_hour_departure: options.return_times.get_departure_hour_max(),
                    min_hour_arrival: options.return_times.get_arrival_hour_min(),
                    max_hour_arrival: options.return_times.get_arrival_hour_max(),
                    stops,
                    max_stopover_minutes,
                    max_duration_minutes,
                };
                leg_vector.push(second_leg);
            }
            TripType::MultiCity => unimplemented!("Multi city trips are not implemented!"),
        }

        leg_vector
    }
}

fn location_to_location_proto(location: &Location) -> LocationProto {
    match &location.loc_type {
        PlaceType::Airport => LocationProto {
            r#type: 1,
            place_name: location.loc_identifier.clone(),
        },
        PlaceType::City => LocationProto {
            r#type: 2,
            place_name: location.loc_identifier.clone(),
        },
        x => panic!("PlaceType type not implemented {:?}", x),
    }
}
impl From<&Config> for ItineraryUrl {
    fn from(options: &Config) -> Self {
        let trip_type = match options.trip_type {
            TripType::Return => 1,
            TripType::OneWay => 2,
            TripType::MultiCity => panic!("Multi City trips are not implemented"),
        };

        let class = options.travel_class as i32;

        let travellers = options.travellers.to_proto_vec();

        let legs: Vec<Leg> = options.into();
        Self {
            legs,
            travellers,
            class,
            trip_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works_airports_oneway() {
        let departure = Location {
            loc_identifier: "MXP".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let destination = Location {
            loc_identifier: "CDG".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };

        let config = Config {
            departing_date: NaiveDate::parse_from_str("2024-05-01", "%Y-%m-%d").unwrap(),
            departure,
            destination,
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::OneWay,
            ..Default::default()
        };

        let it_url = ItineraryUrl::from(&config);

        println!("{}", it_url.to_flight_url());
        let expected = "Gh4SCjIwMjQtMDUtMDFqBwgBEgNNWFByBwgBEgNDREdCAQFIAZgBAg";
        assert_eq!(it_url.to_encoded(), expected);
    }
    #[test]
    fn it_works_cities() {
        let departure = Location {
            loc_identifier: "/m/02_286".to_string(),
            loc_type: PlaceType::City,
            location_name: None,
        };
        let destination = Location {
            loc_identifier: "/m/04jpl".to_string(),
            loc_type: PlaceType::City,
            location_name: None,
        };

        let config = Config {
            departing_date: NaiveDate::parse_from_str("2024-05-01", "%Y-%m-%d").unwrap(),
            departure,
            destination,
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::Return,
            ..Default::default()
        };

        let it_url = ItineraryUrl::from(&config);

        println!("{}", it_url.to_flight_url());
        let expected = "GikSCjIwMjQtMDUtMDFqDQgCEgkvbS8wMl8yODZyDAgCEggvbS8wNGpwbBopEgoyMDI0LTA1LTAzagwIAhIIL20vMDRqcGxyDQgCEgkvbS8wMl8yODZCAQFIAZgBAQ";
        assert_eq!(it_url.to_encoded(), expected);
    }

    #[test]
    fn it_works_cities_many() {
        let departure = Location {
            loc_identifier: "/m/02_286".to_string(),
            loc_type: PlaceType::City,
            location_name: None,
        };
        let destination = Location {
            loc_identifier: "/m/04jpl".to_string(),
            loc_type: PlaceType::City,
            location_name: None,
        };

        let config = Config {
            departing_date: NaiveDate::parse_from_str("2024-05-01", "%Y-%m-%d").unwrap(),
            departure,
            destination,
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::Return,
            travellers: parsers::common::Travelers::new(vec![4, 1, 1, 1]),
            ..Default::default()
        };
        let it_url = ItineraryUrl::from(&config);

        println!("{}", it_url.to_flight_url());
        let expected = "GikSCjIwMjQtMDUtMDFqDQgCEgkvbS8wMl8yODZyDAgCEggvbS8wNGpwbBopEgoyMDI0LTA1LTAzagwIAhIIL20vMDRqcGxyDQgCEgkvbS8wMl8yODZCBwEBAQECAwRIAZgBAQ";
        assert_eq!(it_url.to_encoded(), expected);
    }
}
