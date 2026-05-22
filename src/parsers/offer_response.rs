use crate::{
    parsers::common::{decode_inner_object, decode_outer_object, get_idx},
    parsers::flight_response::{ItineraryCost, RawResponseContainerVec, TripCost},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::flight_response::RawResponseContainer;

pub fn create_raw_response_offer_vec(raw_inputs: String) -> Result<OfferRawResponseContainer> {
    let outer: Vec<RawResponseContainerVec> = decode_outer_object(raw_inputs.as_ref())?;
    let inner_objects: Vec<OfferRawResponse> = outer
        .iter()
        .flat_map(|f| &f.resp)
        .flat_map(|f: &RawResponseContainer| f.payload.clone())
        .filter_map(|payload| decode_inner_object(&payload).ok())
        .collect();
    Ok(OfferRawResponseContainer::new(inner_objects))
}

// ---------------------------------------------------------------------------
// OfferRawResponse — Vec<Value> based
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct OfferRawResponse {
    pub offer_container: OfferContainer,
}

impl<'de> Deserialize<'de> for OfferRawResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(OfferRawResponse {
            offer_container: get_idx(&arr, 1)
                .ok_or_else(|| serde::de::Error::custom("missing offer_container at index 1"))?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OfferRawResponseContainer {
    pub response: Vec<OfferRawResponse>,
}

impl OfferRawResponseContainer {
    pub fn new(response: Vec<OfferRawResponse>) -> Self {
        Self { response }
    }
}

impl OfferRawResponse {
    /// Get offers from the responses.
    /// Returns a vector of (partner names, price) tuples.
    pub fn get_offer_prices(&self) -> Option<Vec<(Vec<String>, i32)>> {
        let offers = self.offer_container.offers.as_ref()?;

        let first_result: Vec<(Vec<String>, i32)> = offers
            .iter()
            .map(|f| {
                let names: Vec<String> = f.partner.iter().map(|y| y.name.clone()).collect();
                let price = f.solution_price.as_ref().and_then(|f| f.trip_cost.clone());
                (names, price)
            })
            .filter_map(|f| f.1.map(|inner| (f.0, inner.price)))
            .collect();

        let second_result: Option<Vec<(Vec<String>, i32)>> = offers
            .iter()
            .flat_map(|f| f.specific_ota_options.as_ref())
            .flatten()
            .map(|f| {
                let names: Vec<String> = f.partner.iter().map(|y| y.name.clone()).collect();
                let price = f.solution_price.as_ref().and_then(|f| f.trip_cost.clone());
                (names, price)
            })
            .filter_map(|f| f.1.map(|inner| Some((f.0, inner.price))))
            .collect();

        let mut result: Vec<(Vec<String>, i32)> = Vec::new();
        result.extend(first_result);
        if let Some(x) = second_result {
            result.extend(x)
        }
        Some(result)
    }
}

// ---------------------------------------------------------------------------
// OfferContainer — Vec<Value> based, keep only offers (idx 0)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct OfferContainer {
    pub offers: Option<Vec<Offers>>,
}

impl<'de> Deserialize<'de> for OfferContainer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(OfferContainer {
            offers: get_idx(&arr, 0),
        })
    }
}

// ---------------------------------------------------------------------------
// Offers — Vec<Value> based, keep useful booking fields
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct Offers {
    pub offer_rank: i32,
    pub partner: Vec<Partner>,
    pub specific_ota_options: Option<Vec<Offers>>,
    pub flight_numbers: Vec<FlightNumbers>,
    pub tracking_url_info: Option<BookingLinkComponents>,
    pub solution_price: Option<ItineraryCost>,
    pub other_currency_prices: Option<Vec<TripCost>>,
}

impl<'de> Deserialize<'de> for Offers {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(Offers {
            offer_rank: get_idx(&arr, 0).unwrap_or_default(),
            partner: get_idx(&arr, 1).unwrap_or_default(),
            specific_ota_options: get_idx(&arr, 2),
            flight_numbers: get_idx(&arr, 3).unwrap_or_default(),
            tracking_url_info: get_idx(&arr, 5),
            solution_price: get_idx(&arr, 7),
            other_currency_prices: get_idx(&arr, 8),
        })
    }
}

// ---------------------------------------------------------------------------
// Stable leaf types — left unchanged
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Partner {
    pub short_code: String,
    pub name: String,
    pub name_2: Option<String>,
    pub is_airline: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlightNumbers {
    pub short_code: String,
    pub number: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BookingLinkComponents {
    pub url_base: String,
    unknown1: Option<String>,
    unknown2: LinkComponents,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct LinkComponents {
    url_base: String,
    unknown1: Vec<ClickInfoComponents>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ClickInfoComponents {
    base_char: String,
    travel_protobuf: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_offer() -> Result<()> {
        let datafiles = "test_files/offers_single_line.txt";

        let mystr = fs::read_to_string(datafiles).expect("Cannot read from file");

        let outer: RawResponseContainerVec = serde_json::from_str(mystr.as_ref())?;

        let inner: Result<OfferRawResponse> =
            decode_inner_object(outer.resp[0].payload.as_ref().unwrap());

        assert!(inner.is_ok());
        Ok(())
    }

    #[test]
    fn test_parse_offer_full() -> Result<()> {
        let datafiles = "test_files/offers_full.txt";

        let mystr = fs::read_to_string(datafiles).expect("Cannot read from file");

        let outer: Vec<RawResponseContainerVec> = decode_outer_object(mystr.as_ref())?;
        let inner_objects: Vec<String> = outer
            .into_iter()
            .flat_map(|f| f.resp)
            .flat_map(|f| f.payload)
            .collect();
        let inner: String = inner_objects.into_iter().next().unwrap();
        let result: Result<OfferRawResponse> = decode_inner_object(inner.as_ref());

        assert!(result.is_ok());
        Ok(())
    }
}
