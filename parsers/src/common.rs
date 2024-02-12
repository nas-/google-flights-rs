use core::panic;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::flight_response::{FlightInfo, ItineraryContainer};
pub(crate) const CHARACTERS_TO_ENCODE: &AsciiSet = &CONTROLS
    .add(b'[')
    .add(b']')
    .add(b'"')
    .add(b',')
    .add(b':')
    .add(b'\\');

/// .
/// Decode the outer object. Responses are in the format
/// )]}'
///
/// 112590 <- Lenght of the following line, moreless.
/// Actual data to parse
/// # Errors
/// This function will return an error if if the data is wrong.
pub(crate) fn decode_outer_object<T: for<'a> Deserialize<'a>>(body: &str) -> Result<Vec<T>> {
    // Read line from the BufRead
    let lines: Vec<&str> = body
        .lines()
        .skip(3)
        .step_by(2)
        .filter(|f| f.trim().starts_with(r#"[["wrb.fr""#))
        .collect();

    let results = lines
        .iter()
        .map(|f| {
            let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
                &mut serde_json::Deserializer::from_str(f);
            let result: Result<T, _> = serde_path_to_error::deserialize(jd);
            match result {
                Ok(x) => Ok(x),
                Err(err) => {
                    let path = err.path().to_string();
                    println!("Error in processing outer object!\n{}\n{:?}", path, err);
                    Err(anyhow!(err))
                }
            }
        })
        .filter(|f| f.is_ok())
        .collect();
    results
}
/// .
/// Decode the inner object
/// The outer object is two values and a 3rd which is data + a JSON value as a string.
/// That is parsed and given out as an output.
/// This function will return an error if if the data is wrong, it errors out.
pub(crate) fn decode_inner_object<T: for<'a> Deserialize<'a>>(body: &str) -> Result<T> {
    // Parse inner object as JSON
    let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
        &mut serde_json::Deserializer::from_str(body);
    let result: Result<T, _> = serde_path_to_error::deserialize(jd);
    match result {
        Ok(x) => Ok(x),
        Err(err) => {
            let path = err.path().to_string();
            println!("Error in processing inner object!\n{}\n{:?}", path, err);
            Err(anyhow!(err))
        }
    }
}

/// Allows to treat empty values as None.
/// This is needed because for some values, sometimes the api returns
/// null and some other times []
pub(crate) fn object_empty_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    for<'a> T: Deserialize<'a>,
{
    #[derive(Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    struct Empty {}

    #[derive(Deserialize, Debug)]
    #[serde(untagged)]
    enum Aux<T> {
        T(T),
        Empty(Empty),
        Null,
    }

    match Deserialize::deserialize(deserializer)? {
        Aux::T(t) => Ok(Some(t)),
        Aux::Empty(_) | Aux::Null => Ok(None),
    }
}
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Copy)]
#[repr(i32)]
#[serde(untagged)]
pub enum PlaceType {
    Unspecified = 0,
    Airport = 1,
    MaybeRegion = 3,
    RegionMaybe = 4,
    City = 5,
}

impl From<i32> for PlaceType {
    fn from(value: i32) -> Self {
        match value {
            0 => PlaceType::Unspecified,
            1 => PlaceType::Airport,
            3 => PlaceType::MaybeRegion,
            4 => PlaceType::RegionMaybe,
            5 => PlaceType::City,
            _ => panic!("Place type can only be 0,1,3,4,5, found {}", value),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum MaybeStringOrInt {
    IntValue(i32),
    StringArray(String),
    IntVector(Vec<i32>),
    None,
    Bool(bool),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum VecOrI32 {
    IntValue(i32),
    VecI32(Vec<Option<i32>>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum NumbersOrBools {
    IntValue(i32),
    Bools(bool),
    None,
}

pub trait ToRequestBody {
    fn to_request_body(&self) -> RequestBody;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestBody {
    pub url: String,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum)]
pub enum TravelClass {
    Economy = 1,
    PremiumEconomy = 2,
    Business = 3,
    First = 4,
}

impl SerializeToWeb for TravelClass {
    fn serialize_to_web(&self) -> String {
        format!("{}", *self as i32)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum)]
pub enum StopOptions {
    All = 0,
    NoStop = 1,
    OneOrLess = 2,
    TwoOrLess = 3,
}

impl SerializeToWeb for StopOptions {
    fn serialize_to_web(&self) -> String {
        format!("{}", *self as i32)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Travelers {
    pub adults: i32,
    pub children: i32,
    pub infant_on_lap: i32,
    pub infant_in_seat: i32,
}

impl Travelers {
    pub fn new(travellers: Vec<i32>) -> Self {
        if travellers[0] < 0 || !travellers.len() == 4 {
            panic!("At least an adult traveller is needed!")
        }

        Self {
            adults: travellers[0],
            children: travellers[1],
            infant_on_lap: travellers[2],
            infant_in_seat: travellers[3],
        }
    }
}

impl SerializeToWeb for Travelers {
    fn serialize_to_web(&self) -> String {
        format!(
            r#"[{},{},{},{}]"#,
            self.adults, self.children, self.infant_on_lap, self.infant_in_seat
        )
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum StopoverDuration {
    Minutes(u32),
    UNLIMITED,
}

impl StopoverDuration {
    pub fn to_option(&self) -> Option<u32> {
        match *self {
            Self::Minutes(i) => Some(i),
            Self::UNLIMITED => None,
        }
    }
    pub fn to_i32(&self) -> Option<i32> {
        match *self {
            Self::Minutes(i) => Some(i as i32),
            Self::UNLIMITED => None,
        }
    }
}

impl SerializeToWeb for StopoverDuration {
    // Google ui allow stopover max to be checked in 30 mins intervals.
    fn serialize_to_web(&self) -> String {
        match self {
            StopoverDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return format!("{}", mins.div_ceil(30) * 30);
                }
                format!("{mins}")
            }
            StopoverDuration::UNLIMITED => "null".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum TotalDuration {
    Minutes(u32),
    UNLIMITED,
}

impl TotalDuration {
    pub fn to_option(&self) -> Option<u32> {
        match *self {
            Self::Minutes(i) => Some(i),
            Self::UNLIMITED => None,
        }
    }
}

impl SerializeToWeb for TotalDuration {
    // Google ui allow total_length max to be checked in 30 mins intervals.
    // total_length is within parentesis
    fn serialize_to_web(&self) -> String {
        match self {
            TotalDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return format!("[{}]", mins.div_ceil(30) * 30);
                }
                format!("[{mins}]")
            }
            TotalDuration::UNLIMITED => "null".to_string(),
        }
    }
}

///[0,23,13,23] --> Leave between 0:00 and 23:59. Arrival between 13 and 23:59
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FlightTimes {
    departure_hour_min: Option<u32>,
    departure_hour_max: Option<u32>,
    arrival_hour_min: Option<u32>,
    arrival_hour_max: Option<u32>,
}
impl FlightTimes {
    pub fn new(
        departure_hour_min: u32,
        departure_hour_max: u32,
        arrival_hour_min: u32,
        arrival_hour_max: u32,
    ) -> Self {
        let min_hour_departure = match departure_hour_min {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let max_departure_hour = match departure_hour_max {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let min_hour_arrival = match arrival_hour_min {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let max_hour_arrival = match arrival_hour_max {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };

        Self {
            departure_hour_min: min_hour_departure,
            departure_hour_max: max_departure_hour,
            arrival_hour_min: min_hour_arrival,
            arrival_hour_max: max_hour_arrival,
        }
    }

    pub fn get_departure_hour_min(&self) -> Option<u32> {
        self.departure_hour_min
    }

    pub fn get_departure_hour_max(&self) -> Option<u32> {
        self.departure_hour_max
    }

    pub fn get_arrival_hour_min(&self) -> Option<u32> {
        self.arrival_hour_min
    }

    pub fn get_arrival_hour_max(&self) -> Option<u32> {
        self.arrival_hour_max
    }
}

impl SerializeToWeb for FlightTimes {
    fn serialize_to_web(&self) -> String {
        if self.departure_hour_min.is_none()
            && self.departure_hour_max.is_none()
            && self.arrival_hour_min.is_none()
            && self.arrival_hour_max.is_none()
        {
            "null".to_string()
        } else {
            format!(
                "[{},{},{},{}]",
                self.departure_hour_min.unwrap_or(0),
                self.departure_hour_max.unwrap_or(23),
                self.arrival_hour_min.unwrap_or(0),
                self.arrival_hour_max.unwrap_or(23)
            )
        }
    }
}

pub trait SerializeToWeb {
    fn serialize_to_web(&self) -> String;
}

impl<T> SerializeToWeb for Vec<T>
where
    T: SerializeToWeb,
{
    fn serialize_to_web(&self) -> String {
        let mut result = String::new();
        result.push('[');

        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            result.push_str(&item.serialize_to_web());
        }

        result.push(']');

        result
    }
}

/// 0 is generic
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Location {
    pub loc_identifier: String,
    pub loc_type: PlaceType,
    // This field is not present in flight response. Just add it so to have a name for the locations.
    #[serde(default)]
    pub location_name: Option<String>,
}

impl Location {
    pub fn new(loc_identifier: &str, loc_type: i32, location_name: Option<String>) -> Self {
        Self {
            loc_identifier: loc_identifier.to_owned(),
            loc_type: loc_type.into(),
            location_name,
        }
    }
}

impl SerializeToWeb for Location {
    fn serialize_to_web(&self) -> String {
        //TODO seems like there is a mismatch here. Maybe it is variant -1 as i32.
        // However, seems with type = 5 it works properly...
        match self.loc_type {
            PlaceType::Airport => format!(r#"[\"{}\",{}]"#, &self.loc_identifier, 0_i32),
            _ => format!(r#"[\"{}\",{}]"#, &self.loc_identifier, self.loc_type as i32),
        }
    }
}

impl SerializeToWeb for &Location {
    fn serialize_to_web(&self) -> String {
        (*self).serialize_to_web()
    }
}

#[derive(Clone)]
pub struct FixedFlights {
    flights: Arc<Mutex<Vec<ItineraryContainer>>>,
    max_elements: usize,
}

impl FixedFlights {
    pub fn new(max_elements: usize) -> Self {
        FixedFlights {
            flights: Arc::new(Mutex::new(Vec::new())),
            max_elements,
        }
    }

    pub fn add_element(&self, element: ItineraryContainer) -> Result<()> {
        if self.is_full() {
            return Err(anyhow!("Vector max number of elements reached"));
        }

        let mut flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(anyhow!("Failed to acquire lock")),
        };
        flights.push(element);
        Ok(())
    }

    pub fn maybe_get_nth_flight_info(&self, nth: usize) -> Option<Vec<FlightInfo>> {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => panic!("Mutext is locked! cannot get is full"),
        };

        let nth = flights.get(nth);
        nth.map(|f| f.itinerary.flight_details.clone())
    }

    pub fn is_full(&self) -> bool {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => panic!("Mutext is locked! cannot get is full"),
        };
        flights.len() >= self.max_elements
    }

    pub fn get_departure_token(&self) -> Option<String> {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => panic!("Mutext is locked! Cannot get token"),
        };
        let length = flights.len().checked_sub(1)?;
        let nth = flights.get(length);

        nth.map(|f| f.get_departure_token())
    }
}
