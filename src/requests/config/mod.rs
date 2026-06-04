use crate::{
    parsers::{self, common::FixedFlights},
    protos::{self, urls::Location as LocationProto},
};
use chrono::{Months, NaiveDate};
use parsers::common::{
    AirlineFilter, FlightTimes, Location, PlaceType, SortOrder, StopOptions, StopoverDuration,
    TotalDuration, TravelClass, Travelers,
};
use protos::urls::{ItineraryUrl, Leg};

mod builder;
mod currency;
pub mod explore;
pub mod multi_city;

pub use builder::ConfigBuilder;
pub use currency::Currency;
pub use explore::{
    ExploreConfig, ExploreDate, ExploreDuration, ExploreResult, Interest, MapBounds,
};
pub use multi_city::{LegFilters, MultiCityConfig, MultiCityConfigBuilder, MultiCityLeg};

/// The `TripType` enum is used to specify the type of trip.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TripType {
    #[default]
    OneWay,
    Return,
    MultiCity,
}

/// The `Config` struct is used to specify the options for a flight search.
///
/// `departure` and `destination` each hold 1–7 airports.  When multiple
/// airports are supplied Google Flights treats them as "any of these" for
/// that end of the journey (e.g. all London-area airports as the origin).
#[derive(Debug, Clone)]
pub struct Config {
    pub departing_date: NaiveDate,
    /// One to seven departure airports / city identifiers.
    pub departure: Vec<Location>,
    /// One to seven destination airports / city identifiers.
    pub destination: Vec<Location>,
    pub stop_options: StopOptions,
    pub travel_class: TravelClass,
    pub return_date: Option<NaiveDate>,
    pub travellers: Travelers,
    pub departing_times: FlightTimes,
    pub return_times: FlightTimes,
    pub stopover_max: StopoverDuration,
    /// Minimum layover / connection time (default: no minimum).
    pub stopover_min: StopoverDuration,
    pub duration_max: TotalDuration,
    pub trip_type: TripType,
    pub currency: Currency,
    pub fixed_flights: FixedFlights,
    /// BCP-47 language subtag for the Google Flights UI, e.g. `"en"`, `"fr"`.
    pub language: String,
    /// ISO 3166-1 alpha-2 country code for locale, e.g. `"GB"`, `"FR"`.
    pub country: String,
    /// Sort order applied to the search results.
    pub sort_order: SortOrder,
    /// Airlines / alliances to include (position \[4\] of the per-leg array).
    /// Empty = no restriction.
    pub airlines_include: Vec<AirlineFilter>,
    /// Airlines / alliances to exclude (position \[5\] of the per-leg array).
    /// Empty = no restriction.
    pub airlines_exclude: Vec<AirlineFilter>,
    /// Require a connection through these IATA airport codes (position \[9\]).
    /// Empty = no restriction.
    pub connecting_airports: Vec<String>,
    /// If `true`, restrict results to lower-CO₂ emissions flights (position \[13\]).
    pub lower_emissions: bool,
    /// Maximum price filter (outer itinerary array position \[7\]). `None` = no price cap.
    pub max_price: Option<i32>,
    /// Baggage filter `(carry_on_count, checked_count)` (outer itinerary array position \[10\]).
    /// `None` = no restriction.
    pub baggage: Option<(u8, u8)>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            departing_date: NaiveDate::default(),
            departure: Vec::new(),
            destination: Vec::new(),
            stop_options: StopOptions::default(),
            travel_class: TravelClass::default(),
            return_date: None,
            travellers: Travelers::default(),
            departing_times: FlightTimes::default(),
            return_times: FlightTimes::default(),
            stopover_max: StopoverDuration::default(),
            stopover_min: StopoverDuration::default(),
            duration_max: TotalDuration::default(),
            trip_type: TripType::default(),
            currency: Currency::default(),
            fixed_flights: FixedFlights::default(),
            language: "en".to_string(),
            country: "GB".to_string(),
            sort_order: SortOrder::default(),
            airlines_include: Vec::new(),
            airlines_exclude: Vec::new(),
            connecting_airports: Vec::new(),
            lower_emissions: false,
            max_price: None,
            baggage: None,
        }
    }
}

impl Config {
    /// Creates a new `Config` object with the specified options.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        departing_date: NaiveDate,
        departure: Vec<Location>,
        destination: Vec<Location>,
        stop_options: StopOptions,
        travel_class: TravelClass,
        return_date: Option<NaiveDate>,
        travellers: Travelers,
        departing_times: FlightTimes,
        return_times: FlightTimes,
        stopover_max: StopoverDuration,
        duration_max: TotalDuration,
        currency: Option<Currency>,
    ) -> Self {
        let trip_type = match return_date {
            Some(_) => TripType::Return,
            None => TripType::OneWay,
        };
        // MultiCity is unreachable here: trip_type is derived solely from
        // return_date above (Some → Return, None → OneWay), so MultiCity can
        // never be produced by this function.
        let fixed_flights = match trip_type {
            TripType::Return => FixedFlights::new(2),
            TripType::OneWay => FixedFlights::new(1),
            TripType::MultiCity => unreachable!(
                "Config::new() never produces TripType::MultiCity; \
                 use a dedicated multi-city builder when that feature is implemented"
            ),
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
            stopover_min: StopoverDuration::default(),
            duration_max,
            trip_type,
            currency: currency.unwrap_or_default(),
            fixed_flights,
            language: "en".to_string(),
            country: "GB".to_string(),
            sort_order: SortOrder::default(),
            airlines_include: Vec::new(),
            airlines_exclude: Vec::new(),
            connecting_airports: Vec::new(),
            lower_emissions: false,
            max_price: None,
            baggage: None,
        }
    }

    /// Calculates the number of days between the departing and return dates, if this is a return trip.
    pub fn get_diff_days(&self) -> Option<i64> {
        self.return_date
            .map(|x| x.signed_duration_since(self.departing_date).num_days())
    }

    /// Calculates the end date of the graph given the number of months to add to the departing date.
    ///
    /// Returns `None` if the resulting date would overflow `NaiveDate`'s range
    /// (only reachable for dates within a few years of the maximum representable date).
    pub fn get_end_graph(&self, months: Months) -> Option<NaiveDate> {
        self.departing_date.checked_add_months(months)
    }

    /// Returns the Google flight URL for this flight search.
    pub fn to_flight_url(&self) -> String {
        ItineraryUrl::from(self).to_flight_url()
    }

    /// Returns the encoded string relative to this flight search.
    pub fn to_encoded(&self) -> String {
        ItineraryUrl::from(self).to_encoded()
    }

    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// Conversion to Leg Protobuf, used for creating the URL.
impl From<&Config> for Vec<Leg> {
    fn from(options: &Config) -> Vec<Leg> {
        let stops = match options.stop_options {
            StopOptions::All => None,
            variant => Some(variant as i32 - 1),
        };
        let max_stopover_minutes = options.stopover_max.to_option();
        let max_duration_minutes = options.duration_max.to_option();

        let departure: Vec<LocationProto> =
            options.departure.iter().map(LocationProto::from).collect();
        let destination: Vec<LocationProto> = options
            .destination
            .iter()
            .map(LocationProto::from)
            .collect();

        let first_leg = Leg {
            date: options.departing_date.to_string(),
            departure: departure.clone(),
            arrival: destination.clone(),
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
                    date: options
                        .return_date
                        .expect("return_date is always Some when TripType is Return")
                        .to_string(),
                    departure: destination.clone(),
                    arrival: departure.clone(),
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
            TripType::MultiCity => unreachable!(
                "MultiCity Config cannot be created via Config::new() or Config::builder(); \
                 construct Config directly only for OneWay or Return trips"
            ),
        }

        leg_vector
    }
}

/// Converts a [`Location`] to the protobuf [`LocationProto`] used in URL encoding.
///
/// Airports use type `1`; cities, regions, and unspecified types use type `2`,
/// which matches how Google Flights encodes region searches.
impl From<&Location> for LocationProto {
    fn from(location: &Location) -> Self {
        let loc_type = match location.loc_type {
            PlaceType::Airport => 1,
            _ => 2,
        };
        LocationProto {
            r#type: loc_type,
            place_name: location.loc_identifier.clone(),
        }
    }
}

/// Conversion from Config to ItineraryUrl, used for creating the URL.
impl From<&Config> for ItineraryUrl {
    fn from(options: &Config) -> Self {
        let trip_type = match options.trip_type {
            TripType::Return => 1,
            TripType::OneWay => 2,
            TripType::MultiCity => unreachable!(
                "MultiCity Config cannot be created via Config::new() or Config::builder(); \
                 construct Config directly only for OneWay or Return trips"
            ),
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn future_date(days: i64) -> NaiveDate {
        Utc::now().date_naive() + Duration::days(days)
    }

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
            departure: vec![departure],
            destination: vec![destination],
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::OneWay,
            ..Default::default()
        };

        let it_url = ItineraryUrl::from(&config);

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
            departure: vec![departure],
            destination: vec![destination],
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::Return,
            ..Default::default()
        };

        let it_url = ItineraryUrl::from(&config);

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
            departure: vec![departure],
            destination: vec![destination],
            return_date: Some(NaiveDate::parse_from_str("2024-05-03", "%Y-%m-%d").unwrap()),
            trip_type: TripType::Return,
            travellers: parsers::common::Travelers::new(vec![4, 1, 1, 1])
                .expect("valid traveler counts"),
            ..Default::default()
        };
        let it_url = ItineraryUrl::from(&config);

        let expected = "GikSCjIwMjQtMDUtMDFqDQgCEgkvbS8wMl8yODZyDAgCEggvbS8wNGpwbBopEgoyMDI0LTA1LTAzagwIAhIIL20vMDRqcGxyDQgCEgkvbS8wMl8yODZCBwEBAQECAwRIAZgBAQ";
        assert_eq!(it_url.to_encoded(), expected);
    }

    #[test]
    fn test_location_to_location_proto_airport() {
        let location = Location {
            loc_type: PlaceType::Airport,
            loc_identifier: "JFK".to_string(),
            location_name: None,
        };
        let location_proto = LocationProto::from(&location);
        assert_eq!(location_proto.r#type, 1);
        assert_eq!(location_proto.place_name, "JFK");
    }

    #[test]
    fn test_location_to_location_proto_city() {
        let location = Location {
            loc_type: PlaceType::City,
            loc_identifier: "/m/04jpl".to_string(),
            location_name: None,
        };
        let location_proto = LocationProto::from(&location);
        assert_eq!(location_proto.r#type, 2);
        assert_eq!(location_proto.place_name, "/m/04jpl");
    }

    // -----------------------------------------------------------------------
    // Multi-airport unit tests
    // -----------------------------------------------------------------------

    /// Helper: build a list of airport [`Location`]s from IATA codes.
    fn airports(codes: &[&str]) -> Vec<Location> {
        codes
            .iter()
            .map(|c| Location {
                loc_identifier: (*c).to_owned(),
                loc_type: PlaceType::Airport,
                location_name: None,
            })
            .collect()
    }

    #[test]
    fn builder_accumulates_airports_up_to_seven() {
        // Google Flights accepts up to 7 airports per side.
        let config = Config {
            departing_date: future_date(30),
            departure: airports(&["LHR", "LGW", "STN", "LTN", "LCY", "SEN", "BHX"]),
            destination: airports(&["JFK"]),
            trip_type: TripType::OneWay,
            ..Default::default()
        };
        assert_eq!(config.departure.len(), 7);
        assert_eq!(config.destination.len(), 1);
    }

    #[test]
    fn multi_departure_produces_seven_proto_entries() {
        let codes = ["LHR", "LGW", "STN", "LTN", "LCY", "SEN", "BHX"];
        let config = Config {
            departing_date: future_date(30),
            departure: airports(&codes),
            destination: airports(&["JFK"]),
            trip_type: TripType::OneWay,
            ..Default::default()
        };
        let legs: Vec<protos::urls::Leg> = (&config).into();
        assert_eq!(legs.len(), 1);
        assert_eq!(
            legs[0].departure.len(),
            7,
            "all seven origin airports should appear as separate proto entries"
        );
        let got: Vec<&str> = legs[0]
            .departure
            .iter()
            .map(|l| l.place_name.as_str())
            .collect();
        for c in codes {
            assert!(got.contains(&c), "missing {c} in proto entries");
        }
    }

    #[test]
    fn multi_departure_produces_two_proto_entries() {
        let config = Config {
            departing_date: future_date(30),
            departure: vec![
                Location {
                    loc_identifier: "LHR".to_owned(),
                    loc_type: PlaceType::Airport,
                    location_name: None,
                },
                Location {
                    loc_identifier: "LGW".to_owned(),
                    loc_type: PlaceType::Airport,
                    location_name: None,
                },
            ],
            destination: vec![Location {
                loc_identifier: "JFK".to_owned(),
                loc_type: PlaceType::Airport,
                location_name: None,
            }],
            trip_type: TripType::OneWay,
            ..Default::default()
        };
        let legs: Vec<protos::urls::Leg> = (&config).into();
        assert_eq!(legs.len(), 1);
        assert_eq!(
            legs[0].departure.len(),
            2,
            "both LHR and LGW should appear as separate proto entries"
        );
        let codes: Vec<&str> = legs[0]
            .departure
            .iter()
            .map(|l| l.place_name.as_str())
            .collect();
        assert!(codes.contains(&"LHR"));
        assert!(codes.contains(&"LGW"));
    }

    #[test]
    fn multi_airport_return_trip_swaps_legs() {
        let config = Config {
            departing_date: future_date(30),
            departure: vec![
                Location {
                    loc_identifier: "LHR".to_owned(),
                    loc_type: PlaceType::Airport,
                    location_name: None,
                },
                Location {
                    loc_identifier: "LGW".to_owned(),
                    loc_type: PlaceType::Airport,
                    location_name: None,
                },
            ],
            destination: vec![Location {
                loc_identifier: "JFK".to_owned(),
                loc_type: PlaceType::Airport,
                location_name: None,
            }],
            return_date: Some(future_date(37)),
            trip_type: TripType::Return,
            ..Default::default()
        };
        let legs: Vec<protos::urls::Leg> = (&config).into();
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].departure.len(), 2);
        assert_eq!(legs[0].arrival.len(), 1);
        assert_eq!(legs[0].arrival[0].place_name, "JFK");
        assert_eq!(legs[1].departure.len(), 1);
        assert_eq!(legs[1].departure[0].place_name, "JFK");
        assert_eq!(legs[1].arrival.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Config::new()
    // -----------------------------------------------------------------------

    #[test]
    fn config_new_oneway_sets_correct_trip_type() {
        let dep = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let dst = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let date = future_date(30);
        let travelers = crate::parsers::common::Travelers::new(vec![1, 0, 0, 0]).unwrap();
        let cfg = Config::new(
            date,
            vec![dep],
            vec![dst],
            StopOptions::default(),
            TravelClass::default(),
            None, // one-way
            travelers,
            FlightTimes::default(),
            FlightTimes::default(),
            StopoverDuration::default(),
            TotalDuration::default(),
            None,
        );
        assert!(matches!(cfg.trip_type, TripType::OneWay));
        assert!(cfg.return_date.is_none());
    }

    #[test]
    fn config_new_return_sets_correct_trip_type() {
        let dep = Location {
            loc_identifier: "MXP".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let dst = Location {
            loc_identifier: "CDG".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let date = future_date(30);
        let ret = future_date(37);
        let travelers = crate::parsers::common::Travelers::new(vec![2, 0, 0, 0]).unwrap();
        let cfg = Config::new(
            date,
            vec![dep],
            vec![dst],
            StopOptions::default(),
            TravelClass::default(),
            Some(ret),
            travelers,
            FlightTimes::default(),
            FlightTimes::default(),
            StopoverDuration::default(),
            TotalDuration::default(),
            Some(Currency::Euro),
        );
        assert!(matches!(cfg.trip_type, TripType::Return));
        assert_eq!(cfg.get_diff_days(), Some(7));
    }

    #[test]
    fn config_get_end_graph_adds_months() {
        use chrono::{Datelike, Months};
        let cfg = Config {
            departing_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            ..Default::default()
        };
        let end = cfg.get_end_graph(Months::new(3)).unwrap();
        assert_eq!(end.month(), 4);
        assert_eq!(end.year(), 2026);
    }
}
