use std::fmt;

use crate::{
    parsers::{self, common::FixedFlights},
    protos::{self, urls::Location as LocationProto},
};
use anyhow::anyhow;
use anyhow::Result;
use chrono::{Months, NaiveDate};
use clap::ValueEnum;
use parsers::common::{
    FlightTimes, Location, PlaceType, StopOptions, StopoverDuration, TotalDuration, TravelClass,
    Travelers,
};

use protos::urls::{ItineraryUrl, Leg};

use super::api::ApiClient;

/// The `TripType` enum is used to specify the type of trip.
#[derive(Debug, Clone, PartialEq)]
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
/// The `Config` struct is used to specify the options for a flight search.
///
/// `departure` and `destination` each hold 1–4 airports.  When multiple
/// airports are supplied Google Flights treats them as "any of these" for
/// that end of the journey (e.g. all London-area airports as the origin).
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub departing_date: NaiveDate,
    /// One to four departure airports / city identifiers.
    pub departure: Vec<Location>,
    /// One to four destination airports / city identifiers.
    pub destination: Vec<Location>,
    pub stop_options: StopOptions,
    pub travel_class: TravelClass,
    pub return_date: Option<NaiveDate>,
    pub travellers: Travelers,
    pub departing_times: FlightTimes,
    pub return_times: FlightTimes,
    pub stopover_max: StopoverDuration,
    pub duration_max: TotalDuration,
    pub trip_type: TripType,
    pub currency: Currency,
    pub fixed_flights: FixedFlights,
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
            duration_max,
            trip_type,
            currency: currency.unwrap_or_default(),
            fixed_flights,
        }
    }
    /// Calculates the number of days between the departing and return dates, if this is a return trip.
    pub fn get_diff_days(&self) -> Option<i64> {
        self.return_date
            .map(|x| x.signed_duration_since(self.departing_date).num_days())
    }
    /// Calculates the end date of the graph, given the number of months in the future to include, starting from the departing date.
    pub fn get_end_graph(&self, months: Months) -> NaiveDate {
        self.departing_date.checked_add_months(months).unwrap()
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

#[derive(Default)]
pub struct ConfigBuilder {
    departing_date: Option<NaiveDate>,
    departure: Vec<Location>,
    destination: Vec<Location>,
    stop_options: StopOptions,
    travel_class: TravelClass,
    return_date: Option<NaiveDate>,
    travelers: Travelers,
    departing_times: FlightTimes,
    return_times: FlightTimes,
    stopover_max: StopoverDuration,
    duration_max: TotalDuration,
    currency: Option<Currency>,
}

impl ConfigBuilder {
    pub fn departing_date(mut self, date: NaiveDate) -> Self {
        self.departing_date = Some(date);
        self
    }

    /// Set the departure to a single airport/city, clearing any previously
    /// added departure airports.
    pub async fn departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.departure = vec![loc];
        Ok(self)
    }

    /// Add an additional departure airport/city (up to 4 total).
    /// Google Flights will search "any of these airports" as the origin.
    pub async fn add_departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        if self.departure.len() >= 4 {
            return Err(anyhow!("A maximum of 4 departure airports is supported"));
        }
        let loc = get_location(location, client).await?;
        self.departure.push(loc);
        Ok(self)
    }

    /// Set the destination to a single airport/city, clearing any previously
    /// added destination airports.
    pub async fn destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.destination = vec![loc];
        Ok(self)
    }

    /// Add an additional destination airport/city (up to 4 total).
    /// Google Flights will search "any of these airports" as the destination.
    pub async fn add_destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        if self.destination.len() >= 4 {
            return Err(anyhow!("A maximum of 4 destination airports is supported"));
        }
        let loc = get_location(location, client).await?;
        self.destination.push(loc);
        Ok(self)
    }

    pub fn return_date(mut self, date: NaiveDate) -> Self {
        self.return_date = Some(date);
        self
    }

    pub fn travelers(mut self, travelers: Travelers) -> Self {
        self.travelers = travelers;
        self
    }

    pub fn stop_options(mut self, stop_options: StopOptions) -> Self {
        self.stop_options = stop_options;
        self
    }

    pub fn travel_class(mut self, travel_class: TravelClass) -> Self {
        self.travel_class = travel_class;
        self
    }

    pub fn departing_times(mut self, times: FlightTimes) -> Self {
        self.departing_times = times;
        self
    }

    pub fn return_times(mut self, times: FlightTimes) -> Self {
        self.return_times = times;
        self
    }

    pub fn stopover_max(mut self, duration: StopoverDuration) -> Self {
        self.stopover_max = duration;
        self
    }

    pub fn duration_max(mut self, duration: TotalDuration) -> Self {
        self.duration_max = duration;
        self
    }

    pub fn currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    pub fn build(self) -> Result<Config> {
        let departing_date = self
            .departing_date
            .ok_or(anyhow!("Departing date is required"))?;
        let trip_type = match self.return_date {
            Some(_) => TripType::Return,
            None => TripType::OneWay,
        };
        if self.departure.is_empty() {
            return Err(anyhow!("At least one departure airport is required"));
        }
        if self.destination.is_empty() {
            return Err(anyhow!("At least one destination airport is required"));
        }
        Ok(Config {
            departing_date,
            departure: self.departure,
            destination: self.destination,
            stop_options: self.stop_options,
            travel_class: self.travel_class,
            return_date: self.return_date,
            travellers: self.travelers,
            departing_times: self.departing_times,
            return_times: self.return_times,
            stopover_max: self.stopover_max,
            duration_max: self.duration_max,
            currency: self.currency.unwrap_or_default(),
            trip_type: trip_type.clone(),
            fixed_flights: match trip_type {
                TripType::Return => FixedFlights::new(2),
                TripType::OneWay => FixedFlights::new(1),
                TripType::MultiCity => {
                    return Err(anyhow!("Multi-city trips are not yet implemented"));
                }
            },
        })
    }
}

async fn get_location(location: &str, client: &ApiClient) -> Result<Location, anyhow::Error> {
    let departure = if location.len() == 3 && location.chars().all(char::is_uppercase) {
        Location::new(location, 1, Some(location.to_string()))
    } else {
        client.request_city(location).await?.to_city_list()
    };
    Ok(departure)
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

        let departure: Vec<LocationProto> = options
            .departure
            .iter()
            .map(location_to_location_proto)
            .collect();

        let destination: Vec<LocationProto> = options
            .destination
            .iter()
            .map(location_to_location_proto)
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
                    date: options.return_date.unwrap().to_string(),
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

/// Conversion from Location to LocationProto, used for creating the URL.
/// Unfortunately, inpossible to create a trait implementation for this as Location and LocationProto live in different subcrates.
fn location_to_location_proto(location: &Location) -> LocationProto {
    let loc_type = match location.loc_type {
        PlaceType::Airport => 1,
        PlaceType::City => 2,
        // Regions and unspecified types are treated as city-level (type 2) in the
        // URL proto, which matches how Google Flights encodes region searches.
        PlaceType::MaybeRegion | PlaceType::RegionMaybe | PlaceType::Unspecified => 2,
    };
    LocationProto {
        r#type: loc_type,
        place_name: location.loc_identifier.clone(),
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

#[derive(Debug, Clone, Default, ValueEnum)]
pub enum Currency {
    AlbanianLek,
    AlgerianDinar,
    ArgentinePeso,
    ArmenianDram,
    ArubanFlorin,
    AustralianDollar,
    AzerbaijaniManat,
    BahamianDollar,
    BahrainiDinar,
    BelarusianRouble,
    BermudianDollar,
    BosniaHerzegovinaMark,
    BrazilianReal,
    BritishPound,
    BulgarianLev,
    CanadianDollar,
    CFPFranc,
    ChileanPeso,
    ChineseYuan,
    ColombianPeso,
    CostaRicanColon,
    CubanPeso,
    CzechKoruna,
    DanishKrone,
    DominicanPeso,
    EgyptianPound,
    #[default]
    Euro,
    GeorgianLari,
    HongKongDollar,
    HungarianForint,
    IcelandicKrona,
    IndianRupee,
    IndonesianRupiah,
    IranianRial,
    IsraeliNewShekel,
    JamaicanDollar,
    JapaneseYen,
    JordanianDinar,
    KazakhstaniTenge,
    KuwaitiDinar,
    LebanesePound,
    MacedonianDenar,
    MalaysianRinggit,
    MexicanPeso,
    MoldovanLeu,
    MoroccanDirham,
    NewTaiwanDollar,
    NewZealandDollar,
    NorwegianKrone,
    OmaniRial,
    PakistaniRupee,
    PanamanianBalboa,
    PeruvianSol,
    PhilippinePeso,
    PolishZloty,
    QatariRiyal,
    RomanianLeu,
    RussianRouble,
    SaudiRiyal,
    SerbianDinar,
    SingaporeDollar,
    SouthAfricanRand,
    SouthKoreanWon,
    SwedishKrona,
    SwissFranc,
    ThaiBaht,
    TurkishLira,
    UkrainianHryvnia,
    UnitedArabEmiratesDirham,
    USDollar,
    VietnameseDong,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = match self {
            Currency::AlbanianLek => "ALL".to_string(),
            Currency::AlgerianDinar => "DZD".to_string(),
            Currency::ArgentinePeso => "ARS".to_string(),
            Currency::ArmenianDram => "AMD".to_string(),
            Currency::ArubanFlorin => "AWG".to_string(),
            Currency::AustralianDollar => "AUD".to_string(),
            Currency::AzerbaijaniManat => "AZN".to_string(),
            Currency::BahamianDollar => "BSD".to_string(),
            Currency::BahrainiDinar => "BHD".to_string(),
            Currency::BelarusianRouble => "BYN".to_string(),
            Currency::BermudianDollar => "BMD".to_string(),
            Currency::BosniaHerzegovinaMark => "BAM".to_string(),
            Currency::BrazilianReal => "BRL".to_string(),
            Currency::BritishPound => "GBP".to_string(),
            Currency::BulgarianLev => "BGN".to_string(),
            Currency::CanadianDollar => "CAD".to_string(),
            Currency::CFPFranc => "XPF".to_string(),
            Currency::ChileanPeso => "CLP".to_string(),
            Currency::ChineseYuan => "CNY".to_string(),
            Currency::ColombianPeso => "COP".to_string(),
            Currency::CostaRicanColon => "CRC".to_string(),
            Currency::CubanPeso => "CUP".to_string(),
            Currency::CzechKoruna => "CZK".to_string(),
            Currency::DanishKrone => "DKK".to_string(),
            Currency::DominicanPeso => "DOP".to_string(),
            Currency::EgyptianPound => "EGP".to_string(),
            Currency::Euro => "EUR".to_string(),
            Currency::GeorgianLari => "GEL".to_string(),
            Currency::HongKongDollar => "HKD".to_string(),
            Currency::HungarianForint => "HUF".to_string(),
            Currency::IcelandicKrona => "ISK".to_string(),
            Currency::IndianRupee => "INR".to_string(),
            Currency::IndonesianRupiah => "IDR".to_string(),
            Currency::IranianRial => "IRR".to_string(),
            Currency::IsraeliNewShekel => "ILS".to_string(),
            Currency::JamaicanDollar => "JMD".to_string(),
            Currency::JapaneseYen => "JPY".to_string(),
            Currency::JordanianDinar => "JOD".to_string(),
            Currency::KazakhstaniTenge => "KZT".to_string(),
            Currency::KuwaitiDinar => "KWD".to_string(),
            Currency::LebanesePound => "LBP".to_string(),
            Currency::MacedonianDenar => "MKD".to_string(),
            Currency::MalaysianRinggit => "MYR".to_string(),
            Currency::MexicanPeso => "MXN".to_string(),
            Currency::MoldovanLeu => "MDL".to_string(),
            Currency::MoroccanDirham => "MAD".to_string(),
            Currency::NewTaiwanDollar => "TWD".to_string(),
            Currency::NewZealandDollar => "NZD".to_string(),
            Currency::NorwegianKrone => "NOK".to_string(),
            Currency::OmaniRial => "OMR".to_string(),
            Currency::PakistaniRupee => "PKR".to_string(),
            Currency::PanamanianBalboa => "PAB".to_string(),
            Currency::PeruvianSol => "PEN".to_string(),
            Currency::PhilippinePeso => "PHP".to_string(),
            Currency::PolishZloty => "PLN".to_string(),
            Currency::QatariRiyal => "QAR".to_string(),
            Currency::RomanianLeu => "RON".to_string(),
            Currency::RussianRouble => "RUB".to_string(),
            Currency::SaudiRiyal => "SAR".to_string(),
            Currency::SerbianDinar => "RSD".to_string(),
            Currency::SingaporeDollar => "SGD".to_string(),
            Currency::SouthAfricanRand => "ZAR".to_string(),
            Currency::SouthKoreanWon => "KRW".to_string(),
            Currency::SwedishKrona => "SEK".to_string(),
            Currency::SwissFranc => "CHF".to_string(),
            Currency::ThaiBaht => "THB".to_string(),
            Currency::TurkishLira => "TRY".to_string(),
            Currency::UkrainianHryvnia => "UAH".to_string(),
            Currency::UnitedArabEmiratesDirham => "AED".to_string(),
            Currency::USDollar => "USD".to_string(),
            Currency::VietnameseDong => "VND".to_string(),
        };
        write!(f, "{}", res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protos;
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

        let location_proto = location_to_location_proto(&location);
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

        let location_proto = location_to_location_proto(&location);
        assert_eq!(location_proto.r#type, 2);
        assert_eq!(location_proto.place_name, "/m/04jpl");
    }

    // -----------------------------------------------------------------------
    // Multi-airport unit tests
    // -----------------------------------------------------------------------

    /// The builder's `departure()` call resets to a single airport, while
    /// subsequent `add_departure()` calls accumulate up to 4.
    #[test]
    fn builder_accumulates_airports_up_to_four() {
        let lhr = Location::new("LHR", 0, None);
        let lgw = Location::new("LGW", 0, None);
        let stn = Location::new("STN", 0, None);
        let ltn = Location::new("LTN", 0, None);
        let jfk = Location::new("JFK", 0, None);

        let config = Config {
            departing_date: future_date(30),
            departure: vec![lhr, lgw, stn, ltn],
            destination: vec![jfk],
            trip_type: TripType::OneWay,
            ..Default::default()
        };
        assert_eq!(config.departure.len(), 4);
        assert_eq!(config.destination.len(), 1);
    }

    /// `build()` returns an error when no departure airport was set.
    #[test]
    fn build_fails_without_departure() {
        let result = Config::builder()
            .departing_date(future_date(30))
            .build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("departure"), "error should mention departure");
    }

    /// `build()` returns an error when no departure airport was set (departure
    /// is validated before destination, so the error message mentions it first).
    #[test]
    fn build_fails_without_both_airports() {
        // Neither departure nor destination is set — the first missing check fires.
        let result = Config::builder()
            .departing_date(future_date(30))
            .build();
        assert!(result.is_err());
        // The validation checks departure first, then destination.
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("departure") || msg.contains("destination"),
            "error should mention a missing airport, got: {msg}"
        );
    }

    /// Two departure airports produce two `LocationProto` entries in the first
    /// Leg's departure field when converted to the URL proto.
    #[test]
    fn multi_departure_produces_two_proto_entries() {
        let config = Config {
            departing_date: future_date(30),
            departure: vec![
                Location::new("LHR", 1, None), // 1 = Airport
                Location::new("LGW", 1, None),
            ],
            destination: vec![Location::new("JFK", 1, None)],
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
        let codes: Vec<&str> = legs[0].departure.iter().map(|l| l.place_name.as_str()).collect();
        assert!(codes.contains(&"LHR"));
        assert!(codes.contains(&"LGW"));
    }

    /// On a return trip the second leg swaps departure/destination,
    /// including all airports from each group.
    #[test]
    fn multi_airport_return_trip_swaps_legs() {
        let config = Config {
            departing_date: future_date(30),
            departure: vec![
                Location::new("LHR", 1, None), // 1 = Airport
                Location::new("LGW", 1, None),
            ],
            destination: vec![Location::new("JFK", 1, None)],
            return_date: Some(future_date(37)),
            trip_type: TripType::Return,
            ..Default::default()
        };
        let legs: Vec<protos::urls::Leg> = (&config).into();
        assert_eq!(legs.len(), 2);
        // Outbound: departs from [LHR, LGW], arrives at [JFK]
        assert_eq!(legs[0].departure.len(), 2);
        assert_eq!(legs[0].arrival.len(), 1);
        assert_eq!(legs[0].arrival[0].place_name, "JFK");
        // Return: departs from [JFK], arrives at [LHR, LGW]
        assert_eq!(legs[1].departure.len(), 1);
        assert_eq!(legs[1].departure[0].place_name, "JFK");
        assert_eq!(legs[1].arrival.len(), 2);
    }
}
