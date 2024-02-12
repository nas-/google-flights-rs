use crate::protos::urls::Location as LocationProto;
use chrono::{Months, NaiveDate};
use parsers::common::{
    FlightTimes, Location, PlaceType, StopOptions, StopoverDuration, TotalDuration, TravelClass,
    Travelers,
};

use protos::urls::Leg;

#[derive(Debug, Clone)]
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
}

impl Config {
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
        }
    }

    pub fn get_diff_days(&self) -> Option<i64> {
        self.return_date
            .map(|x| x.signed_duration_since(self.departing_date).num_days())
    }

    pub fn get_end_graph(&self, months: Months) -> NaiveDate {
        self.departing_date.checked_add_months(months).unwrap()
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

        let departure = match &options.departure.loc_type {
            PlaceType::Airport => LocationProto {
                r#type: 1,
                place_name: options.departure.loc_identifier.clone(),
            },
            PlaceType::City => LocationProto {
                r#type: 2,
                place_name: options.departure.loc_identifier.clone(),
            },
            _ => panic!("Not implemented"),
        };

        let destination = match options.destination.loc_type {
            PlaceType::Airport => LocationProto {
                r#type: 1,
                place_name: options.destination.loc_identifier.clone(),
            },
            PlaceType::City => LocationProto {
                r#type: 2,
                place_name: options.destination.loc_identifier.clone(),
            },
            _ => panic!("Not implemented"),
        };

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

        if options.return_date.is_some() {
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
        };

        leg_vector
    }
}
