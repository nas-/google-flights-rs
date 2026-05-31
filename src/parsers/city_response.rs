use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::parsers::common::{get_idx, GetOuterErrorMessages, Location, PlaceType};

use super::common::{decode_inner_object, decode_outer_object};

// Vec<Value> based — absorbs trailing fields
#[derive(Debug, Serialize)]
struct RawResponseContainer {
    response: RawResponse,
}

impl<'de> Deserialize<'de> for RawResponseContainer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(RawResponseContainer {
            response: get_idx(&arr, 0)
                .ok_or_else(|| serde::de::Error::custom("missing response at index 0"))?,
        })
    }
}

impl GetOuterErrorMessages for RawResponseContainer {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        None
    }
}

// Vec<Value> based — only `body` at index 2 is needed
#[derive(Debug, Serialize)]
struct RawResponse {
    body: String,
}

impl<'de> Deserialize<'de> for RawResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(RawResponse {
            body: get_idx(&arr, 2)
                .ok_or_else(|| serde::de::Error::custom("missing body at index 2"))?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResponseInnerBodyParsed {
    pub result_container: Vec<ResultContainer>,
}

impl TryFrom<&str> for ResponseInnerBodyParsed {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self> {
        let outer: Vec<RawResponseContainer> = decode_outer_object(value)?;
        let inner = &outer
            .first()
            .ok_or_else(|| anyhow!("Malformed data!"))?
            .response
            .body;
        decode_inner_object(inner)
    }
}
///Basically if it is a region, then 5 else 0
impl ResponseInnerBodyParsed {
    pub fn to_city_list(&self) -> Location {
        let bulk = &self.result_container[0];
        if let Some(airport_code) = &bulk.city.airport_code {
            Location {
                loc_identifier: airport_code.clone(),
                loc_type: PlaceType::Airport,
                location_name: Some(bulk.city.city_name.clone()),
            }
        } else {
            Location {
                loc_identifier: bulk.city.identifier.clone(),
                loc_type: PlaceType::City,
                location_name: Some(bulk.city.city_name.clone()),
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResultContainer {
    pub city: PlaceDetails,
    #[serde(default)]
    pub airport: Option<Vec<AirportsNames>>,
}

// Vec<Value> based — extract only the fields used by to_city_list()
#[derive(Debug, Serialize)]
pub struct PlaceDetails {
    pub place_type: PlaceType,
    pub city_name: String,
    pub identifier: String,
    pub airport_code: Option<String>,
}

impl<'de> Deserialize<'de> for PlaceDetails {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(PlaceDetails {
            place_type: get_idx(&arr, 0).unwrap_or_default(),
            city_name: get_idx(&arr, 2).unwrap_or_default(),
            identifier: get_idx(&arr, 4).unwrap_or_default(),
            airport_code: get_idx(&arr, 5),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirportsNames {
    pub airport_info: PlaceDetails,
    distance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response() {
        let raw = r#"a
a
2
[["wrb.fr","H028ib","[[[[3,\"Bedřichov, Czechia\",\"Bedřichov\",\"Municipality in the Czech Republic\",\"/m/05b0wjm\",null,null,null,null,null,null,3],[[[1,\"Václav Havel Airport Prague\",\"Prague\",null,\"/m/05ywg\",\"PRG\",null,null,\"PRG\",null,null,1],\"98 km\"],[[1,\"Pardubice Airport\",\"Pardubice\",null,\"/m/0ch54\",\"PED\",null,null,\"PED\",null,null,1],\"96 km\"],[[5,\"Prague Main Station\",\"Prague\",null,\"/m/05ywg\",\"XYG\",null,null,\"Prague Main Station\",null,null,5],\"92 km\"]]],[[3,\"Bedretto, Switzerland\",\"Bedretto\",\"Municipality in Switzerland\",\"/m/0gxl8h\",null,null,null,null,null,null,3],[[[1,\"Zurich Airport\",\"Zürich\",null,\"/m/08966\",\"ZRH\",null,null,\"ZRH\",null,null,1],\"106 km\"],[[1,\"Milan Malpensa Airport\",\"Milan\",null,\"/m/0947l\",\"MXP\",null,null,\"MXP\",null,null,1],\"98 km\"],[[1,\"Linate Airport\",\"Milan\",null,\"/m/0947l\",\"LIN\",null,null,\"LIN\",null,null,1],\"131 km\"],[[5,\"Lugano\",\"Lugano\",null,\"/m/01r76y\",\"QDL\",null,null,\"Lugano\",null,null,5],\"64 km\"],[[5,\"Zurich HB\",\"Zürich\",null,\"/m/08966\",\"ZLP\",null,null,\"Zurich HB\",null,null,5],\"95 km\"]]],[[3,\"Bedriñana, Spain\",\"Bedriñana\",\"Municipality in Spain\",\"/m/05zvvrs\",null,null,null,null,null,null,3],[[[1,\"Asturias Airport\",\"Aviles\",null,\"/m/044_01\",\"OVD\",null,null,\"OVD\",null,null,1],\"49 km\"],[[1,\"Seve Ballesteros-Santander Airport\",\"Santander\",null,\"/m/016d7r\",\"SDR\",null,null,\"SDR\",null,null,1],\"130 km\"],[[5,\"Oviedo\",\"Oviedo\",null,\"/m/014xj3\",\"OVI\",null,null,\"Oviedo\",null,null,5],\"36 km\"]]],[[3,\"Bedrule, United Kingdom\",\"Bedrule\",\"Hamlet in Scotland\",\"/m/0bwj73c\",null,null,null,null,null,null,3],[[[1,\"Edinburgh Airport\",\"Edinburgh\",null,\"/m/02m77\",\"EDI\",null,null,\"EDI\",null,null,1],\"71 km\"],[[1,\"Newcastle International Airport\",\"Newcastle upon Tyne\",null,\"/m/0j7ng\",\"NCL\",null,null,\"NCL\",null,null,1],\"75 km\"]]],[[3,\"Bedrock, Colorado, USA\",\"Bedrock\",null,\"/m/0271yjn\",null,null,null,null,null,null,3],[[[1,\"Grand Junction Regional Airport\",\"Grand Junction\",null,\"/m/0rb_n\",\"GJT\",null,null,\"GJT\",null,null,1],\"95 km\"],[[1,\"Montrose Regional Airport\",\"Montrose\",null,\"/m/0rc3l\",\"MTJ\",null,null,\"MTJ\",null,null,1],\"89 km\"],[[1,\"Durango-La Plata County Airport\",\"Durango\",null,\"/m/0rbmc\",\"DRO\",null,null,\"DRO\",null,null,1],\"163 km\"],[[1,\"Canyonlands Field Airport\",\"Moab\",null,\"/m/010f5z\",\"CNY\",null,null,\"CNY\",null,null,1],\"89 km\"],[[1,\"Salt Lake City International Airport\",\"Salt Lake City\",null,\"/m/0f2r6\",\"SLC\",null,null,\"SLC\",null,null,1],\"381 km\"]]]]]",null,null,null,"generic"]]"#;
        let raw_parsed: Result<Vec<RawResponseContainer>, _> = decode_outer_object(raw);
        let binding = raw_parsed.unwrap();
        let inner = &binding
            .first()
            .ok_or_else(|| anyhow!("Malformed data!"))
            .unwrap()
            .response
            .body;
        let parsed: Result<ResponseInnerBodyParsed, _> = decode_inner_object(inner);
        assert!(parsed.is_ok());

        let cities: Location = parsed.unwrap().to_city_list();
        assert_eq!(
            cities,
            Location {
                loc_identifier: r"/m/05b0wjm".to_owned(),
                loc_type: PlaceType::City,
                location_name: Some("Bedřichov".to_string())
            }
        )
    }

    #[test]
    fn test_parse_weird_city() {
        let raw = r#"a
        a
        2
        [["wrb.fr","H028ib","[[[[3,\"Pyongyang, North Korea\",\"Pyongyang\",\"Capital of North Korea\",\"/m/0cw5k\",null,null,null,null,null,null,3],[[[1,\"Incheon International Airport\",\"Seoul\",null,\"/m/0hsqf\",\"ICN\",null,null,\"ICN\",null,null,1],\"187 km\"],[[1,\"Gimpo International Airport\",\"Seoul\",null,\"/m/0hsqf\",\"GMP\",null,null,\"GMP\",null,null,1],\"188 km\"],[[1,\"Dandong Langtou Airport\",\"Dandong\",null,\"/m/02wq7t\",\"DDG\",null,null,\"DDG\",null,null,1],\"168 km\"]]]]]",null,null,null,"generic"],["di",70],["af.httprm",69,"5620809061654200627",43]]
        "#;
        let raw_parsed: Result<Vec<RawResponseContainer>, _> = decode_outer_object(raw);
        let binding = raw_parsed.unwrap();
        let inner = &binding
            .first()
            .ok_or_else(|| anyhow!("Malformed data!"))
            .unwrap()
            .response
            .body;
        let parsed: Result<ResponseInnerBodyParsed, _> = decode_inner_object(inner);
        assert!(parsed.is_ok());

        let cities = parsed.unwrap().to_city_list();
        assert_eq!(
            cities,
            Location {
                loc_identifier: r"/m/0cw5k".to_owned(),
                loc_type: PlaceType::City,
                location_name: Some("Pyongyang".to_string())
            }
        )
    }
}
