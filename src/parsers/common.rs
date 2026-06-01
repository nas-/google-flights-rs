use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::ValueEnum;
use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// Extract and deserialize a single element from a JSON array by index.
/// Returns None if the index is out of bounds or the value fails to deserialize.
/// Use this instead of positional serde struct fields so trailing elements
/// added by Google never cause "trailing characters" parse errors.
pub(crate) fn get_idx<T: serde::de::DeserializeOwned>(
    arr: &[serde_json::Value],
    i: usize,
) -> Option<T> {
    arr.get(i)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

use crate::parsers::flight_response::{FlightInfo, ItineraryContainer};

/// The set of characters that are percent-encoded in google flights requests.
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
pub(crate) fn decode_outer_object<T>(body: &str) -> Result<Vec<T>>
where
    T: for<'a> Deserialize<'a> + GetOuterErrorMessages,
{
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
                Ok(x) => {
                    let test = x.get_error_messages();
                    match test {
                    Some(err) => {
                        let err_messages_joined = err.join("\n");
                        tracing::error!(errors = ?err, "Error in processing outer object: errors returned from the backend");
                        Err(anyhow!(err_messages_joined))
                    }
                    None => Ok(x),
                }
            }
                Err(err) => {
                    let path = err.path().to_string();
                    tracing::error!(path = %path, error = ?err, "Error deserializing outer object");
                    Err(anyhow!(err))
                }
            }
        })
        .filter(|f| f.is_ok())
        .collect();
    results
}

/// Decode the inner object
/// The outer object is two values and a 3rd which is data + a JSON value as a string.
/// Following there may be other values, but we are interested only in the 3rd one which contains all the data.
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
            tracing::error!(path = %path, error = ?err, "Error deserializing inner object");
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
    T: std::fmt::Debug,
    for<'a> T: Deserialize<'a>,
{
    use serde::de::{self, Visitor};
    use std::fmt;
    struct RawValueVisitor;

    impl<'de> Visitor<'de> for RawValueVisitor {
        type Value = serde_json::Value;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("any valid JSON value")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_str(v).map_err(de::Error::custom)
        }

        fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_str(v).map_err(de::Error::custom)
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_str(&v).map_err(de::Error::custom)
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_slice(v).map_err(de::Error::custom)
        }

        fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_slice(v).map_err(de::Error::custom)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut elements = Vec::new();
            while let Some(value) = seq.next_element()? {
                elements.push(value);
            }
            Ok(serde_json::Value::Array(elements))
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            let mut values = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry()? {
                values.insert(key, value);
            }
            Ok(serde_json::Value::Object(values))
        }
    }

    #[derive(Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    struct Empty {}

    #[derive(Deserialize, Debug)]
    #[serde(untagged)]
    enum Aux<T> {
        T(T),
        Empty(Empty),
        Null,
        #[allow(dead_code)]
        Array(Vec<serde_json::Value>),
        #[allow(dead_code)]
        Number(serde_json::Number),
    }

    let raw_value: serde_json::Value = deserializer.deserialize_any(RawValueVisitor)?;

    let aux: Aux<T> = serde_json::from_value(raw_value).map_err(de::Error::custom)?;

    match aux {
        Aux::T(t) => Ok(Some(t)),
        Aux::Empty(_) | Aux::Null | Aux::Array(_) | Aux::Number(_) => Ok(None),
    }
}

/// This is the type of place. It can be an airport, a city, a region, etc.
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Copy, Default)]
#[repr(i32)]
#[serde(untagged)]
pub enum PlaceType {
    #[default]
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
            _ => {
                tracing::warn!(
                    value,
                    "Unknown PlaceType discriminant; treating as Unspecified"
                );
                PlaceType::Unspecified
            }
        }
    }
}

/// Trait, serialize the request to a request body, so URL + body.
pub trait ToRequestBody {
    fn to_request_body(&self) -> Result<RequestBody>;
}

/// Url is the url to make the request to
/// Body is the POST request body.
#[derive(Debug, Deserialize, Serialize)]
pub struct RequestBody {
    pub url: String,
    pub body: String,
}

/// Travel class. It can be economy, premium economy, business or first class.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum TravelClass {
    #[default]
    Economy = 1,
    PremiumEconomy = 2,
    Business = 3,
    First = 4,
}

impl SerializeToWeb for TravelClass {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!("{}", *self as i32))
    }
}

impl From<i32> for TravelClass {
    fn from(value: i32) -> Self {
        match value {
            1 => TravelClass::Economy,
            2 => TravelClass::PremiumEconomy,
            3 => TravelClass::Business,
            4 => TravelClass::First,
            _ => {
                tracing::warn!(
                    value,
                    "Unknown TravelClass discriminant; defaulting to Economy"
                );
                TravelClass::Economy
            }
        }
    }
}

/// Sort order for flight search results.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum SortOrder {
    /// Google's default: best combination of price, duration, and convenience.
    #[default]
    Best = 1,
    /// Sort by total price, cheapest first.
    Price = 2,
    /// Sort by total journey duration, shortest first.
    Duration = 3,
    /// Sort by departure time, earliest first.
    DepartureTime = 4,
    /// Sort by arrival time, earliest first.
    ArrivalTime = 5,
}

/// Stop options. It can be all, no stop, one or less, two or less.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum StopOptions {
    #[default]
    All = 0,
    NoStop = 1,
    OneOrLess = 2,
    TwoOrLess = 3,
}

impl SerializeToWeb for StopOptions {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!("{}", *self as i32))
    }
}
impl From<i32> for StopOptions {
    fn from(value: i32) -> Self {
        match value {
            0 => StopOptions::All,
            1 => StopOptions::NoStop,
            2 => StopOptions::OneOrLess,
            3 => StopOptions::TwoOrLess,
            _ => {
                tracing::warn!(value, "Unknown StopOptions discriminant; defaulting to All");
                StopOptions::All
            }
        }
    }
}

/// Travelers. It contains the number of adults, children, infants on lap and infants in seat.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Travelers {
    pub adults: i32,
    pub children: i32,
    pub infant_on_lap: i32,
    pub infant_in_seat: i32,
}
///Default is one adult.
impl Default for Travelers {
    fn default() -> Self {
        Self {
            adults: 1,
            children: 0,
            infant_on_lap: 0,
            infant_in_seat: 0,
        }
    }
}
impl Travelers {
    /// Constructs a [`Travelers`] from a 4-element slice
    /// `[adults, children, infant_on_lap, infant_in_seat]`.
    ///
    /// At least one adult is required and the total number of passengers
    /// must not exceed 9 (Google Flights' server-side limit).
    ///
    /// # Errors
    /// Returns an error if the slice does not have exactly 4 elements,
    /// if `adults < 1`, or if the total exceeds 9.
    pub fn new(travellers: Vec<i32>) -> Result<Self> {
        if travellers.len() != 4 {
            return Err(anyhow!(
                "Travelers::new requires exactly 4 elements \
                 [adults, children, infant_on_lap, infant_in_seat], got {}",
                travellers.len()
            ));
        }
        if travellers[0] < 1 {
            return Err(anyhow!("At least one adult traveller is required"));
        }
        let total: i32 = travellers.iter().sum();
        if total > 9 {
            return Err(anyhow!(
                "Total number of passengers cannot exceed 9, got {}",
                total
            ));
        }
        Ok(Self {
            adults: travellers[0],
            children: travellers[1],
            infant_on_lap: travellers[2],
            infant_in_seat: travellers[3],
        })
    }
    /// Conversion to a vector of i32, used in protobuf generation.
    /// It returns a vector of 1, 2, 3, 4 repeated the number of times of the corresponding field.
    pub fn to_proto_vec(&self) -> Vec<i32> {
        let mut travellers = Vec::new();

        travellers.extend(vec![1; self.adults.try_into().unwrap_or(1)]);
        travellers.extend(vec![2; self.children.try_into().unwrap_or(0)]);
        travellers.extend(vec![3; self.infant_in_seat.try_into().unwrap_or(0)]);
        travellers.extend(vec![4; self.infant_on_lap.try_into().unwrap_or(0)]);
        travellers
    }
}

impl SerializeToWeb for Travelers {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!(
            r#"[{},{},{},{}]"#,
            self.adults, self.children, self.infant_on_lap, self.infant_in_seat
        ))
    }
}
///Stop over duration. It can be a number of minutes or unlimited, with default unlimited.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default)]
pub enum StopoverDuration {
    Minutes(u32),
    #[default]
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
    fn serialize_to_web(&self) -> Result<String> {
        match self {
            StopoverDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return Ok(format!("{}", mins.div_ceil(30) * 30));
                }
                Ok(format!("{mins}"))
            }
            StopoverDuration::UNLIMITED => Ok("null".to_string()),
        }
    }
}

///Total duration. It can be a number of minutes or unlimited, with default unlimited.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub enum TotalDuration {
    Minutes(u32),
    #[default]
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
    fn serialize_to_web(&self) -> Result<String> {
        match self {
            TotalDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return Ok(format!("[{}]", mins.div_ceil(30) * 30));
                }
                Ok(format!("[{mins}]"))
            }
            TotalDuration::UNLIMITED => Ok("null".to_string()),
        }
    }
}
/// Flight times filters. It is the departure hours, and the arrival hours.
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
    fn serialize_to_web(&self) -> Result<String> {
        if self.departure_hour_min.is_none()
            && self.departure_hour_max.is_none()
            && self.arrival_hour_min.is_none()
            && self.arrival_hour_max.is_none()
        {
            Ok("null".to_string())
        } else {
            Ok(format!(
                "[{},{},{},{}]",
                self.departure_hour_min.unwrap_or(0),
                self.departure_hour_max.unwrap_or(23),
                self.arrival_hour_min.unwrap_or(0),
                self.arrival_hour_max.unwrap_or(23)
            ))
        }
    }
}

/// Trait to get the error messages from the response outer messages.
pub trait GetOuterErrorMessages {
    fn get_error_messages(&self) -> Option<Vec<String>>;
}

pub trait SerializeToWeb {
    fn serialize_to_web(&self) -> Result<String>;
}
/// A vector is serialized as a list of elements separated by a comma and enclosed in square brackets.
/// The comma is not added at the end of the last element.
impl<T> SerializeToWeb for Vec<T>
where
    T: SerializeToWeb,
{
    fn serialize_to_web(&self) -> Result<String> {
        let mut result = String::new();
        result.push('[');

        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            result.push_str(&item.serialize_to_web()?);
        }

        result.push(']');

        Ok(result)
    }
}

///Location is a place. It has an identifier (either 3 letter airport code, or google Knowledge graph identifier), a type and a name.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub struct Location {
    pub loc_identifier: String,
    pub loc_type: PlaceType,
    // This field is not present in flight response. Just add it so to have a name for the locations.
    #[serde(default)]
    pub location_name: Option<String>,
}

impl SerializeToWeb for Location {
    fn serialize_to_web(&self) -> Result<String> {
        // Airports are encoded as type 0 in the request body regardless of the
        // PlaceType discriminant; all other location types use their discriminant.
        match self.loc_type {
            PlaceType::Airport => Ok(format!(r#"[\"{}\",{}]"#, &self.loc_identifier, 0_i32)),
            _ => Ok(format!(
                r#"[\"{}\",{}]"#,
                &self.loc_identifier, self.loc_type as i32
            )),
        }
    }
}

impl SerializeToWeb for &Location {
    fn serialize_to_web(&self) -> Result<String> {
        (*self).serialize_to_web()
    }
}

/// Fixed flights is a vector of ItineraryContainer.
/// It has a maximum number of elements, defined by the type of flight that needs to be searched.
///
/// Uses a standard [`std::sync::Mutex`] because all call sites hold the guard only briefly
/// and no async code yields while the lock is held.
#[derive(Clone, Debug)]
pub struct FixedFlights {
    flights: Arc<Mutex<Vec<ItineraryContainer>>>,
    max_elements: usize,
}
impl Default for FixedFlights {
    fn default() -> Self {
        Self::new(2_usize)
    }
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
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; returning None");
                return None;
            }
        };
        flights.get(nth).map(|f| f.itinerary.flight_details.clone())
    }

    pub fn is_full(&self) -> bool {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; assuming not full");
                return false;
            }
        };
        flights.len() >= self.max_elements
    }

    pub fn get_departure_token(&self) -> Option<String> {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; returning None");
                return None;
            }
        };
        let length = flights.len().checked_sub(1)?;
        flights.get(length).map(|f| f.get_departure_token())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travelers_new_valid() {
        let t = Travelers::new(vec![1, 0, 0, 0]).unwrap();
        assert_eq!(t.adults, 1);
    }

    #[test]
    fn travelers_new_wrong_length_errors() {
        assert!(Travelers::new(vec![1, 0, 0]).is_err());
        assert!(Travelers::new(vec![1, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_no_adults_errors() {
        assert!(Travelers::new(vec![0, 1, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_too_many_passengers_errors() {
        // 10 total exceeds server limit of 9
        assert!(Travelers::new(vec![5, 5, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_exactly_nine_passes() {
        assert!(Travelers::new(vec![9, 0, 0, 0]).is_ok());
    }

    #[test]
    fn sort_order_discriminant_values() {
        assert_eq!(SortOrder::Best as i32, 1);
        assert_eq!(SortOrder::Price as i32, 2);
        assert_eq!(SortOrder::Duration as i32, 3);
        assert_eq!(SortOrder::DepartureTime as i32, 4);
        assert_eq!(SortOrder::ArrivalTime as i32, 5);
    }

    #[test]
    fn sort_order_default_is_best() {
        assert!(matches!(SortOrder::default(), SortOrder::Best));
    }
}
