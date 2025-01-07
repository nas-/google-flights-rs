use crate::{
    parsers::common::{decode_inner_object, decode_outer_object},
    parsers::flight_response::{
        CostumerSupport, ItineraryCost, OtherStruct, PriceGraph, RawResponseContainerVec, TripCost,
        Unknown0, VisitedLocation,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn create_raw_response_offer_vec(raw_inputs: String) -> Result<OfferRawResponseContainer> {
    let outer: Vec<RawResponseContainerVec> = decode_outer_object(raw_inputs.as_ref())?;
    let inner_objects: Vec<String> = outer
        .into_iter()
        .flat_map(|f| f.resp)
        .flat_map(|f| f.payload)
        .collect();
    let inner: Vec<OfferRawResponse> = inner_objects
        .into_iter()
        .map(|f| decode_inner_object(&f))
        .filter_map(|f| f.ok())
        .collect();
    Ok(OfferRawResponseContainer::new(inner))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OfferRawResponse {
    unknown0: Unknown0,
    unknown1: OfferContainer,
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
    /// This is a vector of tuples. The first element is a vector of strings (OTA(s)/airline(s)), the second is the price.
    pub fn get_offer_prices(&self) -> Option<Vec<(Vec<String>, i32)>> {
        let first_result: Vec<(Vec<String>, i32)> = self
            .unknown1
            .offers
            .as_ref()?
            .iter()
            .map(|f| {
                let names: Vec<String> = f.partner.iter().map(|y| y.name.clone()).collect();
                let price = &f.solution_price.as_ref().and_then(|f| f.trip_cost.clone());

                (names.clone(), price.clone())
            })
            .filter_map(|f| f.1.map(|inner| (f.0, inner.price)))
            .collect();

        let second_result: Option<Vec<(Vec<String>, i32)>> = self
            .unknown1
            .offers
            .as_ref()?
            .iter()
            .flat_map(|f| f.specific_ota_options.as_ref())
            .flatten()
            .map(|f| {
                let names: Vec<String> = f.partner.iter().map(|y| y.name.clone()).collect();
                let price = &f.solution_price.as_ref().and_then(|f| f.trip_cost.clone());

                (names.clone(), price.clone())
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

#[derive(Debug, Deserialize, Serialize)]
struct OfferContainer {
    offers: Option<Vec<Offers>>,
    unknown1: Option<Vec<Vec<FlightsInfo>>>,
    unknown2: Option<Vec<Vec<Vec<Vec<FlightsInfo>>>>>, //WTF?
    unknown3: Option<String>,
    unknown4: Option<String>,
    unknown5: VisitedLocationContaner,
    unknown6: Option<String>,
    unknown7: Option<Vec<Vec<String>>>,
    unknown8: Option<bool>,
    unknown9: Option<Vec<CostumerSupport>>,
    unknown10: Option<OtaOffers>,
    unknown11: Option<String>,
    unknown12: Option<PriceGraph>,
    unknown13: Option<Vec<i32>>,
    links: BackButtonLinks,
    unknown15: Option<String>,
    unknown16: OtherStruct,
    unknown17: Vec<bool>,
    unknown18: Option<String>,
    unknown19: Option<Vec<i32>>,
    unknown20: Option<String>,
    unknown21: TravelProtobufed,
    unknown22: TravelProtobufed,
}

#[derive(Debug, Deserialize, Serialize)]
struct BackButtonLinks {
    unknown0: Option<String>,
    // links visited in order. first is plain research, second is with first flight selected, ecc.
    links_back: Vec<String>,
    link_hidden_separate_tikets: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct OtaOffers {
    unknown0: Option<i32>,
    ota_offers_by: Vec<String>,
    unknown2: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct InsuranceOptions {
    unknown0: i32,
    #[serde(default)]
    offered_by: String,
    #[serde(default)]
    info_link: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VisitedLocationContaner {
    unknown0: Vec<VisitedLocation>,
    unknown1: i32,
    unknown2: Vec<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Offers {
    offer_rank: i32,
    partner: Vec<Partner>,
    //Recursive....
    specific_ota_options: Option<Vec<Offers>>,
    flight_numbers: Vec<FlightNumbers>,
    unknown4: bool,
    tracking_url_info: Option<BookingLinkComponents>,
    unknown6: Option<String>,
    // Some companies do not show their prices
    solution_price: Option<ItineraryCost>,
    other_currency_prices: Option<Vec<TripCost>>,
    unknown9: Option<InsuranceOptions>,
    unknown10: Option<bool>,
    unknown11: Option<String>,
    unknown12: Option<String>,
    unknown13: Option<ConversionInfo>,
    unknown14: Option<Vec<Vec<Weird1>>>,
    unknown15: Option<String>,
    unknown16: Option<String>,
    unknown17: Option<OtherStruct>,
    unknown18: Option<Vec<MaybeVecOrStruct>>,
    unknown19: Option<String>,
    unknown20: Option<String>,
    unknown21: Option<FlightsInfo>,
    unknown22: Option<FlightsOperator>,
    #[serde(default)]
    unknown23: Option<String>,
    #[serde(default)]
    unknown24: Option<bool>,
    #[serde(default)]
    unknown25: Option<GreenFareInfo>,
    #[serde(default)]
    unknown26: Option<String>,
    #[serde(default)]
    unknown27: Option<Vec<bool>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConversionInfo {
    unknown0: i32,
    unknown1: Vec<TripCost>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GreenFareInfo {
    unknown0: Option<String>,
    unknown1: i32,
    unknown2: i32,
    unknown3: i32,
    unknown4: i32,
    unknown5: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct FlightsInfo {
    unknown0: Option<Vec<String>>,
    unknown1: Option<Vec<Vec<i32>>>,
    unknown2: Option<bool>,
    unknown3: Option<String>,
    unknown4: Option<Vec<MaybeVecOrStruct>>,
    unknown5: Option<String>,
    amenities: Vec<Amenties>,
    unknown7: Option<Value>,
    unknown8: Option<bool>,
    unknown9: Option<String>,
    unknown10: Option<String>,
    unknown11: Vec<Value>,
    unknown12: Option<Vec<VisitedLocation>>,
    unknown13: Option<String>,
    unknown14: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Amenties {
    unknown0: Option<i32>,
    amenities_array: Option<Value>,
    unknown2: Option<i32>,
    legroom_short: Option<String>,
    unknown4: Option<String>,
    legroom_long: Option<String>,
    unknown6: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum MaybeVecOrStruct {
    Struct3(Weird3),
    Struct4(Weird4),
    Struct5(Weird5),
    CurrConversion(CurrencyConversion),
    IntVector(Vec<i32>),
    None,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Weird3 {
    unknown0: i32,
    unknown1: Vec<Currency>,
    unknown2: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Weird4 {
    unknown0: i32,
    unknown1: Vec<Vec<TripCost>>,
    unknown2: i32,
    unknown3: Option<i32>,
    unknown4: Vec<TripCost>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Weird5 {
    unknown0: i32,
    unknown1: Vec<TripCost>,
    unknown2: i32,
    unknown3: Option<i32>,
    unknown4: Vec<TripCost>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FlightsOperator {
    unknown0: Option<String>,
    operator_short_code: Option<String>,
    unknown2: i32,
    unknown3: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CurrencyConversion {
    unknown0: i32,
    unknown1: Vec<Currency>,
    unknown2: i32,
    unknown3: Vec<Currency>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Currency {
    unknown0: Option<String>,
    unknown1: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Weird1 {
    unknown0: Option<Vec<Vec<Vec<String>>>>,
    unknown1: Option<Vec<String>>,
    unknown2: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Partner {
    short_code: String,
    name: String,
    name_2: Option<String>,
    is_airline: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct FlightNumbers {
    short_code: String,
    number: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct BookingLinkComponents {
    url_base: String,
    unknown1: Option<String>,
    unknown2: LinkComponents,
}

#[derive(Debug, Deserialize, Serialize)]
struct LinkComponents {
    url_base: String,
    unknown1: Vec<ClickInfoComponents>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ClickInfoComponents {
    base_char: String,
    travel_protobuf: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TravelProtobufed {
    travel_protobuf: String,
    number: i32,
}

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
        let restult: Result<OfferRawResponse> = decode_inner_object(inner.as_ref());

        assert!(restult.is_ok());
        Ok(())
    }
}
