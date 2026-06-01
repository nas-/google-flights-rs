use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{PlaceType, SerializeToWeb};

/// A location is a place with an identifier (airport code or Google Knowledge Graph ID),
/// a type, and an optional human-readable name.
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::parsers::common::SerializeToWeb;

    use super::*;

    #[test]
    fn location_airport_serializes_with_type_zero() {
        let loc = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        assert_eq!(loc.serialize_to_web().unwrap(), r#"[\"LHR\",0]"#);
    }

    #[test]
    fn location_city_serializes_with_type_five() {
        let loc = Location {
            loc_identifier: "/m/04jpl".to_owned(),
            loc_type: PlaceType::City,
            location_name: Some("London".to_owned()),
        };
        assert_eq!(loc.serialize_to_web().unwrap(), r#"[\"/m/04jpl\",5]"#);
    }
}
