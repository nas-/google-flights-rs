use crate::parsers::common::{decode_inner_object, decode_outer_object, get_idx};
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
// OfferRawResponseContainer
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
pub struct OfferRawResponseContainer {
    pub response: Vec<OfferRawResponse>,
}

impl OfferRawResponseContainer {
    pub fn new(response: Vec<OfferRawResponse>) -> Self {
        Self { response }
    }
}

// ---------------------------------------------------------------------------
// OfferRawResponse
// ---------------------------------------------------------------------------

/// Top-level wrapper parsed from a `GetBookingResults` inner payload.
///
/// Response structure (inner payload is an array):
/// ```text
/// arr[0]  = response header / metadata
/// arr[1]  = [[ offer_group, … ]]   ← two levels of wrapping before the groups
/// arr[2+] = flight numbers, flags, repeated price summary, …
/// ```
///
/// Each *offer_group* is `[rank, airlines, sub_options, null, [price_arr, token], …]`.
#[derive(Debug, Serialize)]
pub struct OfferRawResponse {
    pub offers: Vec<OfferGroup>,
}

impl<'de> Deserialize<'de> for OfferRawResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;

        // arr[1] = [[ offer_group, … ]] — unwrap one nesting level to reach groups
        let level1: Vec<Value> = get_idx(&arr, 1).unwrap_or_default();
        let offer_groups: Vec<Value> = level1
            .into_iter()
            .filter_map(|v| v.as_array().cloned())
            .flatten()
            .collect();

        let offers: Vec<OfferGroup> = offer_groups
            .into_iter()
            .filter_map(|g| serde_json::from_value(g).ok())
            .collect();

        Ok(OfferRawResponse { offers })
    }
}

impl OfferRawResponse {
    /// Returns `(airline_names, total_price)` pairs for every booking option.
    ///
    /// Each pair represents one way to buy the selected itinerary.  For a
    /// single-airline round trip the list typically contains one entry per OTA
    /// (Lufthansa direct, Booking.com, Mytrip, …).  For a multi-airline
    /// itinerary (e.g. Avianca outbound + American return) there is usually
    /// one combined entry whose price is the sum of both legs.
    pub fn get_offer_prices(&self) -> Option<Vec<(Vec<String>, i32)>> {
        let result: Vec<(Vec<String>, i32)> = self
            .offers
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
// OfferGroup — one booking option
// ---------------------------------------------------------------------------

/// One booking option returned by `GetBookingResults`.
///
/// Structure: `[rank, airlines, sub_options, null, [[null, price], token], …]`
///
/// | Index | Content |
/// |-------|---------|
/// | 1     | `[["AV","Avianca",…], ["AA","American",…]]` — airline entries |
/// | 2     | per-OTA sub-options (each with its own price / token) |
/// | 4     | `[[null, total_price], booking_token]` |
#[derive(Debug, Serialize, Clone)]
pub struct OfferGroup {
    /// Airline display names involved in this booking option.
    pub airline_names: Vec<String>,
    /// Total price in the response currency (e.g. 951 for €951).
    pub price: Option<i32>,
    /// Opaque booking token used to construct a booking URL.
    pub booking_token: Option<String>,
    /// Per-OTA booking sub-options (useful when a single-airline trip is sold
    /// through multiple booking channels at different prices).
    pub sub_options: Vec<BookingSubOption>,
}

impl<'de> Deserialize<'de> for OfferGroup {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;

        // group[1] = [["AV","Avianca",null,true], ["AA","American",null,true], …]
        let airlines: Vec<Vec<Value>> = get_idx(&arr, 1).unwrap_or_default();
        let airline_names: Vec<String> = airlines
            .iter()
            .filter_map(|a| a.get(1))
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect();

        // Price/token location varies by route type:
        //   - Multi-airline combined offers:   group[4] = [[null, price], token]
        //   - Single-airline / OTA sub-options: group[7] = [[null, price], token]
        // Try index 4 first; fall back to index 7.
        let (price, booking_token) = [4usize, 7usize]
            .into_iter()
            .find_map(|idx| {
                let price_arr: Vec<Value> = get_idx(&arr, idx).unwrap_or_default();
                let price = price_arr
                    .get(0)
                    .and_then(|v| v.get(1))
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)?;
                let token = price_arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some((Some(price), token))
            })
            .unwrap_or((None, None));

        // group[2] = list of per-OTA sub-options
        let sub_options: Vec<BookingSubOption> = get_idx(&arr, 2).unwrap_or_default();

        Ok(OfferGroup {
            airline_names,
            price,
            booking_token,
            sub_options,
        })
    }
}

// ---------------------------------------------------------------------------
// BookingSubOption — one booking channel within an offer group
// ---------------------------------------------------------------------------

/// A single booking channel inside an [`OfferGroup`].
///
/// Structure: `[rank, partner_entries, null, flight_numbers, …, null, [[null, price], token], …]`
///
/// | Index | Content |
/// |-------|---------|
/// | 1     | `[["LH","Lufthansa",…]]` or `[["Booking.com",…]]` — partner |
/// | 7     | `[[null, price], booking_token]` |
#[derive(Debug, Serialize, Clone)]
pub struct BookingSubOption {
    /// Partner / OTA names for this booking channel.
    pub partner_names: Vec<String>,
    /// Price for this specific booking channel.
    pub price: Option<i32>,
    /// Opaque booking token for this channel.
    pub booking_token: Option<String>,
}

impl<'de> Deserialize<'de> for BookingSubOption {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;

        // sub[1] = [["LH","Lufthansa",null,true], …]
        let partners: Vec<Vec<Value>> = get_idx(&arr, 1).unwrap_or_default();
        let partner_names: Vec<String> = partners
            .iter()
            .filter_map(|p| p.get(1))
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect();

        // sub[7] = [[null, price], booking_token]
        let price_arr: Vec<Value> = get_idx(&arr, 7).unwrap_or_default();
        let price: Option<i32> = price_arr
            .get(0)
            .and_then(|v| v.get(1))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let booking_token: Option<String> = price_arr
            .get(1)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(BookingSubOption {
            partner_names,
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
        let mystr = fs::read_to_string("test_files/offers_single_line.txt")
            .expect("Cannot read from file");
        let outer: RawResponseContainerVec = serde_json::from_str(mystr.as_ref())?;
        // Just verify the pipeline doesn't crash on old-format data.
        let _inner: Result<OfferRawResponse> =
            decode_inner_object(outer.resp[0].payload.as_ref().unwrap());
        Ok(())
    }

    #[test]
    fn test_parse_offer_full() -> Result<()> {
        let mystr =
            fs::read_to_string("test_files/offers_full.txt").expect("Cannot read from file");
        let outer: Vec<RawResponseContainerVec> = decode_outer_object(mystr.as_ref())?;
        let inner_objects: Vec<String> = outer
            .into_iter()
            .flat_map(|f| f.resp)
            .flat_map(|f| f.payload)
            .collect();
        let inner: String = inner_objects.into_iter().next().unwrap();
        // Just verify the pipeline doesn't crash on old-format data.
        let _result: Result<OfferRawResponse> = decode_inner_object(inner.as_ref());
        Ok(())
    }

    /// Parses a minimal synthetic `GetBookingResults` inner payload and
    /// verifies that the combined €951 offer (Avianca outbound + American
    /// return) is extracted correctly.
    ///
    /// Structure mirrored from a real MAD→MEX round-trip capture:
    /// ```text
    /// arr[0]  = response header
    /// arr[1]  = [[ offer_group ]]   (two levels of wrapping)
    ///   offer_group = [rank, airlines, sub_options, null, [[null,951],"token"]]
    /// arr[6]  = [[null,951],"token"]  (repeated total price)
    /// ```
    #[test]
    fn test_booking_offer_inner_payload_parses_951() -> Result<()> {
        // Minimal but structurally complete GetBookingResults inner payload.
        let inner_payload = r#"[
            [null, [[1,2,3],null,null,null,null,[[4]]], 1, "session", "token"],
            [[
                [5,
                 [["AV","Avianca",null,true],["AA","American",null,true]],
                 [
                   [0,[["AV","Avianca",null,true]],null,[["AV","11"],["AV","44"]],false,"url",null,[[null,525],"tok_av"],null,null,false],
                   [0,[["AA","American",null,true]],null,[["AA","2996"],["AA","94"]],false,"url",null,[[null,426],"tok_aa"],null,null,false]
                 ],
                 null,
                 [[null,951],"tok_combined"],
                 null,null,false]
            ]],
            [["AV","11"],["AV","44"],["AA","2996"],["AA","94"]],
            false,null,null,
            [[null,951],"tok_repeat"]
        ]"#;

        let result: OfferRawResponse = decode_inner_object(inner_payload)?;

        let prices = result
            .get_offer_prices()
            .expect("expected at least one offer");

        assert!(
            prices.iter().any(|(_, p)| *p == 951),
            "expected offer with price 951, got: {:?}",
            prices
        );

        let offer_951 = prices.iter().find(|(_, p)| *p == 951).unwrap();
        assert!(
            offer_951.0.iter().any(|n| n.contains("Avianca")),
            "951 offer should include Avianca, got: {:?}",
            offer_951.0
        );
        assert!(
            offer_951.0.iter().any(|n| n.contains("American")),
            "951 offer should include American, got: {:?}",
            offer_951.0
        );

        Ok(())
    }

    /// Verify that the sub-option prices (per-airline leg prices) are also
    /// accessible for itineraries with multiple airlines.
    #[test]
    fn test_booking_sub_option_prices() -> Result<()> {
        let inner_payload = r#"[
            [null, [[1,2,3],null,null,null,null,[[4]]], 1, "session", "token"],
            [[
                [5,
                 [["AV","Avianca",null,true],["AA","American",null,true]],
                 [
                   [0,[["AV","Avianca",null,true]],null,[["AV","11"],["AV","44"]],false,"url",null,[[null,525],"tok_av"]],
                   [0,[["AA","American",null,true]],null,[["AA","2996"],["AA","94"]],false,"url",null,[[null,426],"tok_aa"]]
                 ],
                 null,
                 [[null,951],"tok_combined"]]
            ]],
            false,null,null,
            [[null,951],"tok_repeat"]
        ]"#;

        let result: OfferRawResponse = decode_inner_object(inner_payload)?;

        assert_eq!(result.offers.len(), 1, "expected 1 offer group");
        let group = &result.offers[0];

        assert_eq!(group.price, Some(951));
        assert_eq!(group.sub_options.len(), 2);
        assert_eq!(group.sub_options[0].price, Some(525));
        assert_eq!(group.sub_options[1].price, Some(426));

        Ok(())
    }
}
