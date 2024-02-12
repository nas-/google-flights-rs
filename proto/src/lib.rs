pub mod urls {
    include!(concat!(env!("OUT_DIR"), "/urls.rs"));
}
use base64::{
    alphabet,
    engine::{self, general_purpose},
    Engine as _,
};

use prost::Message;
use urls::ItineraryUrl;
use urls::{Leg, LocType, Location};

const CUSTOM_ENGINE: engine::GeneralPurpose =
    engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);
use std::vec;

impl Leg {
    pub fn from_parts(arrival: &str, departure: &str, date_departure: &str) -> Self {
        Self {
            date: date_departure.into(),
            departure: vec![Location {
                place_name: departure.into(),
                r#type: LocType::City.into(),
            }],
            arrival: vec![Location {
                place_name: arrival.into(),
                r#type: LocType::City.into(),
            }],
            ..Default::default()
        }
    }
}

impl ItineraryUrl {
    pub fn to_encoded(&self) -> String {
        let mut encoded_message = Vec::new();
        self.encode(&mut encoded_message).unwrap();
        CUSTOM_ENGINE.encode(encoded_message)
    }
    pub fn to_flight_url(&self) -> String {
        let encoded = self.to_encoded();

        format!(
            "https://www.google.com/travel/flights/search?tfs={}",
            encoded
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urls::{Leg, LocType, Location, StopOptions, TravelClass, Traveler, TripType};

    #[test]
    fn it_works() {
        let leg1 = Leg {
            date: "2024-11-06".into(),
            arrival: vec![Location {
                place_name: "MXP".into(),
                r#type: LocType::Airport.into(),
            }],
            departure: vec![Location {
                place_name: "CDG".into(),
                r#type: LocType::Airport.into(),
            }],
            stops: None,
            min_hour_departure: None,
            max_hour_departure: None,
            min_hour_arrival: None,
            max_hour_arrival: None,
            max_stopover_minutes: None,
            max_duration_minutes: None,
        };

        let leg2 = Leg {
            date: "2024-11-13".into(),
            arrival: vec![Location {
                place_name: "CDG".into(),
                r#type: LocType::Airport.into(),
            }],
            departure: vec![Location {
                place_name: "MXP".into(),
                r#type: LocType::Airport.into(),
            }],
            stops: None,
            min_hour_departure: None,
            max_hour_departure: None,
            min_hour_arrival: None,
            max_hour_arrival: None,
            max_stopover_minutes: None,
            max_duration_minutes: None,
        };

        let x = ItineraryUrl {
            legs: vec![leg1, leg2],
            travellers: vec![Traveler::Adult.into()],
            class: TravelClass::Economy.into(),
            trip_type: TripType::RoundTrip.into(),
        };

        let base64_encoded_message = x.to_encoded();

        println!("{}", x.to_flight_url());
        let expected = "Gh4SCjIwMjQtMTEtMDZqBwgBEgNDREdyBwgBEgNNWFAaHhIKMjAyNC0xMS0xM2oHCAESA01YUHIHCAESA0NER0IBAUgBmAEB";
        assert_eq!(base64_encoded_message, expected);
    }

    #[test]
    fn it_works_locations() {
        // Ar with 2 adults and 3 different kids from NY to london in first.
        let leg1 = Leg {
            date: "2024-11-06".into(),
            departure: vec![Location {
                place_name: "/m/02_286".into(), //new_york
                r#type: LocType::City.into(),
            }],
            arrival: vec![Location {
                place_name: "/m/04jpl".into(), //London
                r#type: LocType::City.into(),
            }],
            stops: Some(StopOptions::NoStop.into()),
            min_hour_departure: None,
            max_hour_departure: None,
            min_hour_arrival: None,
            max_hour_arrival: None,
            max_stopover_minutes: None,
            max_duration_minutes: None,
        };

        let leg2 = Leg {
            date: "2024-11-13".into(),
            departure: vec![Location {
                place_name: "/m/04jpl".into(), // london
                r#type: LocType::City.into(),
            }],
            arrival: vec![Location {
                place_name: "/m/02_286".into(), //new_york
                r#type: LocType::City.into(),
            }],
            stops: Some(StopOptions::NoStop.into()),
            min_hour_departure: None,
            max_hour_departure: None,
            min_hour_arrival: None,
            max_hour_arrival: None,
            max_stopover_minutes: None,
            max_duration_minutes: None,
        };

        let x = ItineraryUrl {
            legs: vec![leg1, leg2],
            travellers: vec![
                Traveler::Adult.into(),
                Traveler::Adult.into(),
                Traveler::Child.into(),
                Traveler::InfantLap.into(),
                Traveler::InfantSeat.into(),
            ],
            class: TravelClass::First.into(),
            trip_type: TripType::RoundTrip.into(),
        };

        let base64_encoded_message = x.to_encoded();

        println!("{}", x.to_flight_url());

        let expected = "GisSCjIwMjQtMTEtMDYoAGoNCAISCS9tLzAyXzI4NnIMCAISCC9tLzA0anBsGisSCjIwMjQtMTEtMTMoAGoMCAISCC9tLzA0anBscg0IAhIJL20vMDJfMjg2QgUBAQIDBEgEmAEB";
        assert_eq!(base64_encoded_message, expected);
    }

    #[test]
    fn it_works_times() {
        let leg1 = Leg {
            date: "2024-11-06".into(),
            arrival: vec![Location {
                place_name: "MXP".into(),
                r#type: LocType::Airport.into(),
            }],
            departure: vec![Location {
                place_name: "CDG".into(),
                r#type: LocType::Airport.into(),
            }],
            stops: None,
            min_hour_departure: Some(7),
            max_hour_departure: Some(23),
            min_hour_arrival: Some(0),
            max_hour_arrival: Some(23),
            max_stopover_minutes: Some(600),
            max_duration_minutes: Some(600),
        };

        let leg2 = Leg {
            date: "2024-11-13".into(),
            arrival: vec![Location {
                place_name: "CDG".into(),
                r#type: LocType::Airport.into(),
            }],
            departure: vec![Location {
                place_name: "MXP".into(),
                r#type: LocType::Airport.into(),
            }],
            stops: None,
            min_hour_departure: Some(19),
            max_hour_departure: Some(23),
            min_hour_arrival: Some(0),
            max_hour_arrival: Some(23),
            max_stopover_minutes: Some(600),
            max_duration_minutes: Some(600),
        };

        let x = ItineraryUrl {
            legs: vec![leg1, leg2],
            travellers: vec![Traveler::Adult.into()],
            class: TravelClass::Economy.into(),
            trip_type: TripType::RoundTrip.into(),
        };
        let base64_encoded_message = x.to_encoded();
        println!("{}", x.to_flight_url());

        let expected = "Gi0SCjIwMjQtMTEtMDZAB0gXUABYF2DYBGoHCAESA0NER3IHCAESA01YUJAB2AQaLRIKMjAyNC0xMS0xM0ATSBdQAFgXYNgEagcIARIDTVhQcgcIARIDQ0RHkAHYBEIBAUgBmAEB";
        assert_eq!(base64_encoded_message, expected);
    }
}
