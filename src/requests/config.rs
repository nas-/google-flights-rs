use core::panic;
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
    pub currency: Currency,
    pub fixed_flights: FixedFlights,
}

impl Config {
    /// Creates a new `Config` object with the specified options.
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
        currency: Option<Currency>,
    ) -> Self {
        let trip_type = match return_date {
            Some(_) => TripType::Return,
            None => TripType::OneWay,
        };
        let fixed_flights = match trip_type {
            TripType::Return => FixedFlights::new(2),
            TripType::OneWay => FixedFlights::new(1),
            TripType::MultiCity => panic!("Multi city trips are not implemented!"),
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
    departure: Option<Location>,
    destination: Option<Location>,
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

    pub async fn departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let departure = get_location(location, client).await?;
        self.departure = Some(departure);
        Ok(self)
    }

    pub async fn destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let destination = get_location(location, client).await?;
        self.destination = Some(destination);
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
        Ok(Config {
            departing_date,
            departure: self
                .departure
                .ok_or(anyhow!("Departure location is required"))?,
            destination: self
                .destination
                .ok_or(anyhow!("Destination location is required"))?,
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
                TripType::MultiCity => panic!("Multi city trips are not implemented!"),
            },
        })
    }
}

async fn get_location(location: &str, client: &ApiClient) -> Result<Location, anyhow::Error> {
    let departure = if location.len() == 3 && location.chars().all(char::is_uppercase) {
        Location::new(location, 1, None)
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

/// Conversion from Location to LocationProto, used for creating the URL.
/// Unfortunately, inpossible to create a trait implementation for this as Location and LocationProto live in different subcrates.
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
/// Conversion from Config to ItineraryUrl, used for creating the URL.
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
}
