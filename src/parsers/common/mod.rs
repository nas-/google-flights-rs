use anyhow::{anyhow, Result};
use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Deserializer};

pub mod airline;
pub mod duration;
pub mod fixed_flights;
pub mod location;
pub mod travelers;
pub mod types;

pub use airline::{AirlineCode, AirlineFilter, Alliance};
pub use duration::{FlightTimes, StopoverDuration, TotalDuration};
pub use fixed_flights::FixedFlights;
pub use location::Location;
pub use types::{PlaceType, SortOrder, StopOptions, TravelClass};
pub use travelers::Travelers;

/// The set of characters that are percent-encoded in google flights requests.
pub(crate) const CHARACTERS_TO_ENCODE: &AsciiSet = &CONTROLS
    .add(b'[')
    .add(b']')
    .add(b'"')
    .add(b',')
    .add(b':')
    .add(b'\\');

/// Url is the url to make the request to
/// Body is the POST request body.
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct RequestBody {
    pub url: String,
    pub body: String,
}

/// Trait, serialize the request to a request body (URL + body).
pub trait ToRequestBody {
    fn to_request_body(&self) -> Result<RequestBody>;
}

/// Trait to get the error messages from the response outer messages.
pub trait GetOuterErrorMessages {
    fn get_error_messages(&self) -> Option<Vec<String>>;
}

/// Trait to serialize a value into the Google Flights web request format.
pub trait SerializeToWeb {
    fn serialize_to_web(&self) -> Result<String>;
}

/// A vector is serialized as a JSON array.
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

/// Decode the outer object. Responses are in the format
/// `)]}'`
///
/// followed by the actual data (one batch-response line per result).
///
/// # Errors
/// Returns an error if the data is malformed.
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

/// Decode the inner object.
///
/// The outer object is two values and a 3rd which is data + a JSON value as a string.
/// Following there may be other values, but we are interested only in the 3rd one which contains all the data.
/// That is parsed and given out as an output.
///
/// # Errors
/// Returns an error if the data is malformed.
pub(crate) fn decode_inner_object<T: for<'a> Deserialize<'a>>(body: &str) -> Result<T> {
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

/// Allows treating empty values as None.
/// This is needed because for some values, sometimes the API returns
/// `null` and some other times `[]`.
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
