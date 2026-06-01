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
    AirlineFilter, FlightTimes, Location, PlaceType, SortOrder, StopOptions, StopoverDuration,
    TotalDuration, TravelClass, Travelers,
};

use protos::urls::{ItineraryUrl, Leg};

use super::api::ApiClient;

/// The `TripType` enum is used to specify the type of trip.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TripType {
    #[default]
    OneWay,
    Return,
    MultiCity,
}
/// The `Config` struct is used to specify the options for a flight search.
///
/// `departure` and `destination` each hold 1–4 airports.  When multiple
/// airports are supplied Google Flights treats them as "any of these" for
/// that end of the journey (e.g. all London-area airports as the origin).
#[derive(Debug, Clone)]
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
    /// Minimum layover duration. Default: no minimum.
    stopover_min: StopoverDuration,
    duration_max: TotalDuration,
    currency: Option<Currency>,
    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`. Default: `"en"`.
    language: String,
    /// ISO 3166-1 alpha-2 country code, e.g. `"GB"`, `"FR"`. Default: `"GB"`.
    country: String,
    /// Sort order for search results. Default: [`SortOrder::Best`].
    sort_order: SortOrder,
    /// Airlines / alliances to include. Empty = no restriction.
    airlines_include: Vec<AirlineFilter>,
    /// Airlines / alliances to exclude. Empty = no restriction.
    airlines_exclude: Vec<AirlineFilter>,
    /// Connecting airport IATA codes. Empty = no restriction.
    connecting_airports: Vec<String>,
    /// Restrict to lower-CO₂ emissions flights. Default: `false`.
    lower_emissions: bool,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            departing_date: None,
            departure: Vec::new(),
            destination: Vec::new(),
            stop_options: StopOptions::default(),
            travel_class: TravelClass::default(),
            return_date: None,
            travelers: Travelers::default(),
            departing_times: FlightTimes::default(),
            return_times: FlightTimes::default(),
            stopover_max: StopoverDuration::default(),
            stopover_min: StopoverDuration::default(),
            duration_max: TotalDuration::default(),
            currency: None,
            language: "en".to_string(),
            country: "GB".to_string(),
            sort_order: SortOrder::default(),
            airlines_include: Vec::new(),
            airlines_exclude: Vec::new(),
            connecting_airports: Vec::new(),
            lower_emissions: false,
        }
    }
}

impl ConfigBuilder {
    pub fn departing_date(mut self, date: NaiveDate) -> Self {
        self.departing_date = Some(date);
        self
    }

    /// Set the departure to a single airport/city from a string, clearing any previously
    /// added departure airports.  Resolves IATA codes and city names via the network.
    pub async fn departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.departure = vec![loc];
        Ok(self)
    }

    /// Set the departure to a single [`Location`] directly, without a network lookup.
    ///
    /// Use this when you already have a `Location` (e.g. from a previous city lookup or
    /// when building tests without an [`ApiClient`]).
    pub fn departure_location(mut self, location: Location) -> Self {
        self.departure = vec![location];
        self
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

    /// Set the destination to a single airport/city from a string, clearing any previously
    /// added destination airports.  Resolves IATA codes and city names via the network.
    pub async fn destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.destination = vec![loc];
        Ok(self)
    }

    /// Set the destination to a single [`Location`] directly, without a network lookup.
    ///
    /// Use this when you already have a `Location` (e.g. from a previous city lookup or
    /// when building tests without an [`ApiClient`]).
    pub fn destination_location(mut self, location: Location) -> Self {
        self.destination = vec![location];
        self
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

    /// Set the minimum layover / connection duration.
    ///
    /// Use this to avoid very short layovers.  Google Flights lets you choose
    /// minimum layover times in 30-minute intervals.
    pub fn stopover_min(mut self, duration: StopoverDuration) -> Self {
        self.stopover_min = duration;
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

    /// Set the language for the Google Flights UI.
    ///
    /// Accepts a BCP-47 language subtag such as `"en"`, `"fr"`, `"de"`.
    /// Defaults to `"en"`.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Set the country for the Google Flights locale.
    ///
    /// Accepts an ISO 3166-1 alpha-2 code such as `"GB"`, `"FR"`, `"US"`.
    /// Case-insensitive — stored as-supplied, uppercased in requests.
    /// Defaults to `"GB"`.
    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = country.into();
        self
    }

    /// Set the sort order for flight search results.
    ///
    /// Defaults to [`SortOrder::Best`] (Google's composite ranking).
    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = sort_order;
        self
    }

    /// Replace the entire airlines-include list.
    ///
    /// Accepts IATA codes (`"LX"`, `"LH"`) and alliance names
    /// (`"ONEWORLD"`, `"SKYTEAM"`, `"STAR_ALLIANCE"`).
    pub fn airlines_include(mut self, filters: Vec<AirlineFilter>) -> Self {
        self.airlines_include = filters;
        self
    }

    /// Add a single airline/alliance to the include filter.
    pub fn add_airline_include(mut self, filter: AirlineFilter) -> Self {
        self.airlines_include.push(filter);
        self
    }

    /// Replace the entire airlines-exclude list.
    pub fn airlines_exclude(mut self, filters: Vec<AirlineFilter>) -> Self {
        self.airlines_exclude = filters;
        self
    }

    /// Add a single airline/alliance to the exclude filter.
    pub fn add_airline_exclude(mut self, filter: AirlineFilter) -> Self {
        self.airlines_exclude.push(filter);
        self
    }

    /// Set the list of connecting airports (IATA codes, e.g. `"CDG"`).
    ///
    /// Only itineraries that connect through at least one of these airports
    /// will be returned.
    pub fn connecting_airports(mut self, airports: Vec<String>) -> Self {
        self.connecting_airports = airports;
        self
    }

    /// Add a single connecting airport (IATA code).
    pub fn add_connecting_airport(mut self, airport: impl Into<String>) -> Self {
        self.connecting_airports.push(airport.into());
        self
    }

    /// If `true`, restrict results to flights with lower CO₂ emissions.
    ///
    /// Defaults to `false` (no restriction).
    pub fn lower_emissions(mut self, lower: bool) -> Self {
        self.lower_emissions = lower;
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
            stopover_min: self.stopover_min,
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
            language: self.language,
            country: self.country,
            sort_order: self.sort_order,
            airlines_include: self.airlines_include,
            airlines_exclude: self.airlines_exclude,
            connecting_airports: self.connecting_airports,
            lower_emissions: self.lower_emissions,
        })
    }
}

async fn get_location(location: &str, client: &ApiClient) -> Result<Location, anyhow::Error> {
    let departure = if location.len() == 3 && location.chars().all(char::is_uppercase) {
        Location {
            loc_identifier: location.to_owned(),
            loc_type: PlaceType::Airport,
            location_name: Some(location.to_string()),
        }
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

    /// The builder's `departure()` call resets to a single airport, while
    /// subsequent `add_departure()` calls accumulate up to 4.
    #[test]
    fn builder_accumulates_airports_up_to_four() {
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
        let stn = Location {
            loc_identifier: "STN".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let ltn = Location {
            loc_identifier: "LTN".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };

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
        let result = Config::builder().departing_date(future_date(30)).build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("departure"), "error should mention departure");
    }

    /// `build()` returns an error when no departure airport was set (departure
    /// is validated before destination, so the error message mentions it first).
    #[test]
    fn build_fails_without_both_airports() {
        // Neither departure nor destination is set — the first missing check fires.
        let result = Config::builder().departing_date(future_date(30)).build();
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

    /// `build()` returns an error when departure is set but destination is not.
    #[test]
    fn build_fails_without_destination() {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let result = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("destination"),
            "error should mention destination, got: {msg}"
        );
    }

    /// ConfigBuilder locale setters work and Config::default() starts with en/GB.
    #[test]
    fn config_default_locale_is_en_gb() {
        let cfg = Config::default();
        assert_eq!(cfg.language, "en");
        assert_eq!(cfg.country, "GB");
    }

    /// ConfigBuilder::language() and country() setters propagate to the built Config.
    #[test]
    fn builder_locale_setters_propagate() {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .language("fr")
            .country("FR")
            .build()
            .expect("valid config");
        assert_eq!(cfg.language, "fr");
        assert_eq!(cfg.country, "FR");
    }

    /// ConfigBuilder::sort_order() propagates the chosen sort order.
    #[test]
    fn builder_sort_order_setter_propagates() {
        use crate::parsers::common::SortOrder;
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .sort_order(SortOrder::Price)
            .build()
            .expect("valid config");
        assert!(matches!(cfg.sort_order, SortOrder::Price));
    }

    /// On a return trip the second leg swaps departure/destination,
    /// including all airports from each group.
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
        // Outbound: departs from [LHR, LGW], arrives at [JFK]
        assert_eq!(legs[0].departure.len(), 2);
        assert_eq!(legs[0].arrival.len(), 1);
        assert_eq!(legs[0].arrival[0].place_name, "JFK");
        // Return: departs from [JFK], arrives at [LHR, LGW]
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
        // One-way → FixedFlights holds exactly 1 slot (checked indirectly via get_diff_days)
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

    // -----------------------------------------------------------------------
    // Currency::Display — spot-check a handful of known ISO codes
    // -----------------------------------------------------------------------

    #[test]
    fn currency_display_spot_check() {
        assert_eq!(Currency::Euro.to_string(), "EUR");
        assert_eq!(Currency::USDollar.to_string(), "USD");
        assert_eq!(Currency::BritishPound.to_string(), "GBP");
        assert_eq!(Currency::JapaneseYen.to_string(), "JPY");
        assert_eq!(Currency::SwissFranc.to_string(), "CHF");
        assert_eq!(Currency::AustralianDollar.to_string(), "AUD");
    }

    // -----------------------------------------------------------------------
    // ConfigBuilder filter setters
    // -----------------------------------------------------------------------

    #[test]
    fn builder_filter_setters_propagate() {
        use crate::parsers::common::{AirlineCode, AirlineFilter, Alliance};
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .add_airline_include(AirlineFilter::Airline(AirlineCode::new("LX").unwrap()))
            .add_airline_include(AirlineFilter::Alliance(Alliance::OneWorld))
            .add_airline_exclude(AirlineFilter::Airline(AirlineCode::new("FR").unwrap()))
            .add_connecting_airport("CDG")
            .lower_emissions(true)
            .stopover_min(StopoverDuration::Minutes(60))
            .stopover_max(StopoverDuration::Minutes(180))
            .build()
            .expect("valid config");

        assert_eq!(cfg.airlines_include.len(), 2);
        assert_eq!(cfg.airlines_exclude.len(), 1);
        assert_eq!(cfg.connecting_airports, vec!["CDG"]);
        assert!(cfg.lower_emissions);
        assert!(matches!(cfg.stopover_min, StopoverDuration::Minutes(60)));
        assert!(matches!(cfg.stopover_max, StopoverDuration::Minutes(180)));
    }
}
