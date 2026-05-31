use crate::parsers::common::{decode_inner_object, decode_outer_object};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::flight_response::{RawResponseContainer, RawResponseContainerVec};

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
// OfferRawResponse
// ---------------------------------------------------------------------------

/// Top-level wrapper parsed from the inner payload JSON.
///
/// The Google batchexecute response payload is a 30-element array.
/// The offer data lives at index 3: `[offer_list, count, …]`.
#[derive(Debug, Serialize)]
pub struct OfferRawResponse {
    pub offer_container: OfferContainer,
}

impl<'de> Deserialize<'de> for OfferRawResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        // arr[3] = [offer_list, count, flag, flag, [1]]
        let offer_container: OfferContainer = arr
            .get(3)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(OfferContainer { offers: None });
        Ok(OfferRawResponse { offer_container })
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
    /// Returns `(airline_names, price_cents)` pairs for every offer in this response.
    pub fn get_offer_prices(&self) -> Option<Vec<(Vec<String>, i32)>> {
        let offers = self.offer_container.offers.as_ref()?;
        let result: Vec<(Vec<String>, i32)> = offers
            .iter()
            .filter_map(|o| o.price.map(|p| (o.airline_names.clone(), p)))
            .collect();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

// ---------------------------------------------------------------------------
// OfferContainer
// ---------------------------------------------------------------------------

/// Wraps `arr[3]` which is `[offer_list, count, …]`.
/// The offer list is at index 0.
#[derive(Debug, Serialize)]
pub struct OfferContainer {
    pub offers: Option<Vec<Offers>>,
}

impl<'de> Deserialize<'de> for OfferContainer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        // arr[3][0] = the list of individual offer entries
        let offers: Option<Vec<Offers>> = arr
            .get(0)
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        Ok(OfferContainer { offers })
    }
}

// ---------------------------------------------------------------------------
// Offers — one entry in the offer list
// ---------------------------------------------------------------------------

/// Each offer entry inside `arr[3][0]` is an array of the form:
/// ```text
/// [
///   ["AA", ["American"], [[...flights...]], "MEX", ...],  // index 0: flight data
///   [[null, 951], "CjRIdzAx..."],                          // index 1: price + booking token
///   null, true/false, ...
/// ]
/// ```
#[derive(Debug, Serialize, Clone)]
pub struct Offers {
    /// Airline display names, e.g. `["American"]` or `["American", "Iberia"]`.
    pub airline_names: Vec<String>,
    /// Total price in the response currency (e.g. 951 for €951).
    pub price: Option<i32>,
    /// Opaque booking token used to construct a booking URL.
    pub booking_token: Option<String>,
}

impl<'de> Deserialize<'de> for Offers {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;

        // arr[0][1] = ["American"] or ["American","Iberia"]
        let airline_names: Vec<String> = arr
            .get(0)
            .and_then(|v| v.get(1))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // arr[1][0][1] = 951
        let price: Option<i32> = arr
            .get(1)
            .and_then(|v| v.get(0))
            .and_then(|v| v.get(1))
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // arr[1][1] = "CjRIdzAx..."
        let booking_token: Option<String> = arr
            .get(1)
            .and_then(|v| v.get(1))
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Ok(Offers {
            airline_names,
            price,
            booking_token,
        })
    }
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

    /// Parses the real captured offer response and verifies that the 951 price
    /// (and the American Airlines offer) are correctly extracted.
    #[test]
    fn test_parse_raw_offer_response_prices() -> Result<()> {
        let mystr =
            fs::read_to_string("test_files/raw_offer_response.txt").expect("Cannot read file");

        let result: OfferRawResponseContainer = create_raw_response_offer_vec(mystr)?;

        let all_offers: Vec<(Vec<String>, i32)> = result
            .response
            .iter()
            .filter_map(|r| r.get_offer_prices())
            .flatten()
            .collect();

        assert!(!all_offers.is_empty(), "expected at least one offer");

        // The cheapest offer (American Airlines) should be 951.
        let cheapest = all_offers.iter().min_by_key(|(_, p)| p).unwrap();
        assert_eq!(cheapest.1, 951, "cheapest offer price should be 951");
        assert!(
            cheapest.0.iter().any(|n| n.contains("American")),
            "cheapest offer should be American Airlines, got {:?}",
            cheapest.0
        );

        Ok(())
    }
}
