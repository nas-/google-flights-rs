use core::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use crate::parsers::common::GetOuterErrorMessages;
use crate::parsers::common::SerializeToWeb;

use super::common::{decode_inner_object, decode_outer_object, object_empty_as_none, Location};
use anyhow::anyhow;
use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AirplaneInfo {
    pub code: String,
    pub flight_number: String,
    #[serde(default)]
    pub plane_crew_by: Option<String>,
    pub name: String,
}

impl AirplaneInfo {
    pub fn new(
        code: String,
        flight_number: String,
        plane_crew_by: Option<String>,
        name: String,
    ) -> Self {
        Self {
            code,
            flight_number,
            plane_crew_by,
            name,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Date {
    year: i32,
    month: i32,
    day: i32,
}

impl Date {
    pub fn new(year: i32, month: i32, day: i32) -> Self {
        Self { year, month, day }
    }
}
impl SerializeToWeb for Date {
    fn serialize_to_web(&self) -> Result<String> {
        let date = NaiveDate::from_ymd_opt(self.year, self.month as u32, self.day as u32)
            .ok_or_else(|| anyhow!("Invalid date!"))?;
        Ok(date.format("%Y-%m-%d").to_string())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct Hour {
    //hour = None = hour after midnight
    #[serde(default)]
    hour: Option<i32>,
    #[serde(default)]
    minute: i32,
}

impl Hour {
    pub fn new(hour: Option<i32>, minute: i32) -> Self {
        Self { hour, minute }
    }
}

/// The type of seat. One is unknown for now.
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Copy)]
#[repr(i32)]
#[serde(untagged)]
enum SeatType {
    AverageLegroom = 1,
    BelowAverageLegroom = 2,
    AboveAverageLegroom = 3,
    ExtraReclining = 4,
    LieFlat = 5,
    IndividualSuite = 6,
    UnknownSeat = 7,
    StandardReclinerSeat = 8,
    AngledFlatSeat = 9,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Copy)]
#[repr(i32)]
#[serde(untagged)]
enum Wifi {
    None,
    Available = 1,
    Free = 2,
    ForFee = 3,
}
impl Default for Wifi {
    fn default() -> Self {
        Self::None
    }
}
/// Flight amenities vector. The first value meaning is unknown.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
struct FlightAmenities {
    unknown0: Option<Value>,
    power_and_usb_outlets: Option<bool>,
    some_power_and_usb_outlets: Option<bool>,
    in_seat_power_outlet: Option<bool>,
    some_in_seat_power_outlet: Option<bool>,
    in_seat_usb_outlet: Option<bool>,
    some_in_seat_usb_outlet: Option<bool>,
    personal_video_screen: Option<bool>,
    live_tv: Option<bool>,
    on_demand_video: Option<bool>,
    stream_video_to_own_device: Option<bool>,
    wifi: Wifi,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
struct FlightWarnings {
    overnight_flight: Option<bool>,
    delayed_30_mins: Option<bool>,
    different_class_business: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FlightInfo {
    unknown0: Option<String>,
    unknown1: Option<String>,
    operated_by: Option<String>,
    departure_airport_code: String,
    departure_airport_name: Option<String>,
    destination_airport_name: Option<String>,
    destination_airport_code: String,
    unknown7: Option<i32>,
    departure_time: Hour,
    unknown9: Option<i32>,
    arrival_time: Hour,
    duration: Option<i32>,
    flight_amenities: Option<FlightAmenities>, //invalid type: integer `3`, expected a boolean for one way.
    seat_type: Option<SeatType>,
    legroom_short: Option<String>,
    code_share_flight_numbers: Option<Vec<Vec<Option<String>>>>, //[["LH", "9448", null, "Lufthansa"]]
    // Bag carry allowance?
    unknown16: Option<i32>,
    aircraft: Option<String>,
    flight_warnings: Option<FlightWarnings>,
    unknown19: Option<bool>,
    departure_date: Date,
    arrival_date: Date,
    airplane_info: AirplaneInfo,
    unknown23: Option<String>,
    unknown24: Option<Unknown24>,
    // Bag carry allowance?
    unknown25: Option<i32>,
    #[serde(default)]
    unknown26: Option<String>,
    #[serde(default)]
    unknown27: Option<String>,
    #[serde(default)]
    unknown28: Option<String>,
    #[serde(default)]
    unknown29: Option<String>,
    #[serde(default)]
    legroom_long: Option<String>,
    #[serde(default)]
    carbon_emission_mg: Option<i64>,
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct Unknown24 {
    unknown0: Option<i32>,
    #[serde(default)]
    unknown1: Option<Unknown25>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct Unknown25 {
    unknown0: Option<Value>,
    #[serde(default)]
    unknown1: Value,
}

impl FlightInfo {
    pub fn new(
        departure_airport_code: String,
        destination_airport_code: String,
        departure_time: Hour,
        arrival_time: Hour,
        departure_date: Date,
        arrival_date: Date,
        airplane_info: AirplaneInfo,
    ) -> Self {
        Self {
            departure_airport_code,
            destination_airport_code,
            departure_time,
            arrival_time,
            departure_date,
            arrival_date,
            airplane_info,
            ..Default::default()
        }
    }
}

impl SerializeToWeb for FlightInfo {
    fn serialize_to_web(&self) -> Result<String> {
        //TODO why do i have to escape each of the values?
        Ok(format!(
            r#"[\"{}\",\"{}\",\"{}\",null,\"{}\",\"{}\"]"#,
            self.departure_airport_code,
            self.departure_date.serialize_to_web()?,
            self.destination_airport_code,
            self.airplane_info.code,
            self.airplane_info.flight_number
        ))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OtherStruct {
    // [1705070296848121, 139803069, 858572], None, None, None, None, [[2]]
    numbers: Vec<i64>,
    unknown2: Option<String>,
    unknown3: Option<String>,
    unknown4: Option<String>,
    unknown5: Option<String>,
    unknown6: Vec<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct Emissions {
    // [None, None, 1, -9, None, True, True, 78000, 86000, None, 119000, 1, False]
    unknown0: Option<String>,
    unknown1: Option<String>,
    unknown2: Option<i64>,
    emission_vs_average_percent: Option<i64>,
    unknown4: Option<String>,
    unknown5: Option<bool>,
    unknown6: Option<bool>,
    co2_this_flight_g: Option<i64>,
    co2_typical_route_g: Option<i64>,
    unknown9: Option<Vec<Option<bool>>>,
    co2_lowest_route_g: i64,
    unknown11: i64,
    unknown12: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CostumerSupport {
    // ['LX', 'SWISS', 'https://www.swiss.com/gb/en/prepare/special-care']]
    company_two_letter_code: String,
    company_extended_name: String,
    assistance_link: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ConnectionInfo {
    // [[75, "ZRH", "ZRH", null, "Zurich Airport", "Z\u00c3\u00bcrich", "Zurich Airport", "Z\u00c3\u00bcrich"]]
    connection_time_minutes: i32,
    arrival_airport: String,
    departure_airport: String,
    // 1 = Overnight Layover
    connection_warnings: Option<Vec<i32>>,
    arriving_airport_name: Option<String>,
    arriving_city: Option<String>,
    departure_airport_name: Option<String>,
    departure_city: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Itinerary {
    pub flight_by: String,
    operated_by: Option<Vec<String>>,
    pub flight_details: Vec<FlightInfo>,
    departure_airport_code: String,
    departure_date: Date,
    departure_hour: Hour,
    arrival_airport_code: String,
    arrival_date: Date,
    arrival_hour: Hour,
    total_time_minutes: i64,
    unknown10: Option<i32>,
    unknown11: Option<String>,
    unknown12: Option<bool>,
    connection_airport_info: Option<Vec<ConnectionInfo>>,
    unknown14: Option<String>,
    unknown15: Option<ItinUnknown15>,
    airlines_with_codeshare: Option<Vec<String>>,
    unknown17: Option<String>,
    unknown18: OtherStruct,
    unknown19: i64,
    unknown20: Option<String>,
    unknown21: Option<String>,
    #[serde(default)]
    emissions: Option<Emissions>,
    //Bag allowance?
    #[serde(default)]
    unknown23: Vec<i64>,
    #[serde(default)]
    passenger_assistance_links: Option<Vec<CostumerSupport>>,
    #[serde(default)]
    unknown25: Option<String>,
    #[serde(default)]
    unknown26: Value,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
struct ItinUnknown15 {
    unknown0: Option<i32>,
    #[serde(default)]
    unknown1: Option<Vec<ItinUnknown16>>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
struct ItinUnknown16 {
    unknown0: Option<i32>,
    unknown1: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ItineraryCost {
    //[[None, 138], 'CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEgxMWDc1MXxMWDE2MjgaCgihaxACGgNFVVI4HHDDdQ==']
    #[serde(deserialize_with = "object_empty_as_none")]
    pub trip_cost: Option<TripCost>,
    //use this in the following requests to block this departure flight
    departure_token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TripCost {
    unknown: Option<String>,
    pub price: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ItineraryContainer {
    pub itinerary: Itinerary,
    pub itinerary_cost: ItineraryCost,
    unknown2: Option<String>,
    unknown3: bool,
    unknown4: Value,
    unknown5: Vec<bool>,
    unknown6: bool,
    #[serde(deserialize_with = "object_empty_as_none")]
    itinerary_warnings: Option<ItineraryWarnings>, //ok 1303 /[2, 'Mytrip', 'https://mytrip.com/rf/self-transfer']
    departure_protobuf: String,
    unknown9: Option<Vec<Vec<Option<i32>>>>,
    unknown10: bool,
}

impl ItineraryContainer {
    pub fn get_departure_token(&self) -> String {
        self.itinerary_cost.departure_token.clone()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ItineraryWarnings {
    id_or: Value,
    company_name: String,
    warning_link: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItineraryContainerList {
    pub itinerary_list: Vec<ItineraryContainer>,
    unknown1: Option<i32>,
    unknown2: bool,
    unknown3: bool,
    unknown4: Vec<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Unknown15 {
    unknown0: i32,
    unknown1: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Unknown12 {
    unknown0: String,
    unknown1: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct AllianceMappings {
    unknown0: Option<Vec<Vec<Option<i32>>>>,
    alliances: Vec<Vec<Vec<String>>>,
    alliances_hubs: Option<AllianceHubs>,
    unknown3: Option<Vec<i32>>,
    unknown4: Option<Vec<Vec<i32>>>,
    unknown5: Option<Vec<bool>>,
    unknown6: Option<String>,
    unknown7: Vec<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AllianceHubs {
    hub_mapping: Vec<Vec<String>>,
    #[serde(default)]
    unknown1: i32,
    #[serde(default)]
    unknown2: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct CheaperTravelDifferentDatesContainer {
    different_dates: Option<CheaperTravelDifferentDates>,
    different_airport: Option<StartFromOtherAirportOption>, //string or soemthing else.
    unknown2: Option<OtherStruct>,
    #[serde(default)]
    unknown3: Option<OtherStruct>,
    #[serde(default)]
    different_airport_or_dates: Option<CheaperTravelDifferentPlaces>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CheaperTravelDifferentPlaces {
    #[serde(default)]
    dates: Option<Vec<CheaperTravelDifferentDates>>,
    #[serde(default)]
    airports: Option<Vec<StartFromOtherAirportOption>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TripCostContainer {
    //[[None, 31], 'CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEh9taWNyb2ZsZXh8YWdnOjIwMjQwMTI3LTIwMjQwMTMwGgoImhgQAhoDRVVSOBxwvho=']
    pub trip_cost: TripCost,
    cost_protobuf: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CheaperTravelDifferentDates {
    // ['2024-01-27', '2024-01-30', [[None, 31], 'CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEh9taWNyb2ZsZXh8YWdnOjIwMjQwMTI3LTIwMjQwMTMwGgoImhgQAhoDRVVSOBxwvho='], None, [1, 1]]
    pub proposed_departure_date: NaiveDate,
    proposed_return_date: Option<NaiveDate>,
    pub proposed_trip_cost: Option<TripCostContainer>,
    unknown3: Option<Value>,
    #[serde(default)]
    unknown4: Vec<i32>,
}
impl CheaperTravelDifferentDates {
    pub fn maybe_get_date_price(&self) -> Option<(NaiveDate, i32)> {
        self.proposed_trip_cost
            .as_ref()
            .map(|f| (self.proposed_departure_date, f.trip_cost.price))
    }
}

impl Display for CheaperTravelDifferentDates {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(return_date) = self.proposed_return_date {
            write!(
                f,
                "Proposed departure date: {}, proposed return date: {}, proposed trip cost: {}",
                self.proposed_departure_date,
                return_date,
                self.proposed_trip_cost
                    .as_ref()
                    .map(|f| f.trip_cost.price)
                    .unwrap_or_default()
            )
        } else {
            write!(
                f,
                "One way, proposed departure date: {}, proposed trip cost: {}",
                self.proposed_departure_date,
                self.proposed_trip_cost
                    .as_ref()
                    .map(|f| f.trip_cost.price)
                    .unwrap_or_default()
            )
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct StartFromOtherAirportOption {
    // ['CDG', True, [[None, 2028], 'CjRIOENVaUxnZ3dpUTRBQWthN1FCRy0tLS0tLS0tLS13ZWsyOUFBQUFBR1dsWUlJTWJLb0FBEiRtaWNyb2ZsZXh8YWdnfG5lYXJieTpkZXBhcnR1cmUtQ0RHLTEaCwiesAwQAhoDRVVSOBxwg8cN'], '/m/05qtj', '274 km']
    departure_airport: String,
    unknown1: bool,
    proposed_trip_cost: Option<TripCostContainer>,
    departure_place: Option<String>,
    distance: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriceGraph {
    unknown0: i32,
    current_lowest_price: TripCost,
    lowest_hist_price: TripCost,
    lowest_price_days_ago: Vec<Option<i32>>,
    pub usual_price_low_bound: TripCost,
    usual_price_high_bound: TripCost,
    unknown6: i32,
    unknown7: Option<Value>,
    unknown8: Option<String>,
    unknown9: Option<String>,
    price_graph: Option<Vec<Vec<PricePoint>>>,
    unknown11: OtherStruct,
    destination_city_name: String,
    #[serde(default)]
    cheapest_to_book: Option<CheapestBook>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CheapestBook {
    unknown0: Option<SimilarDate>,
    unknown1: SimilarDate,
    unknown2: SimilarDate,
    in_average_cheaper: TripCost,
}

#[derive(Debug, Deserialize, Serialize)]
struct SimilarDate {
    unknown0: Vec<i32>,
    unknown1: i32,
    #[serde(default)]
    unknown2: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PricePoint {
    price_epoch: i64,
    price_point: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Unknown0 {
    unknown0: Option<String>,
    unknown1: Option<OtherStruct>, // [[1705063796213762, 139803069, 858572], None, None, None, None, [[1]]],
    unknown2: i32,
    unknown3: String,
    unknown4: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CityImages {
    pub destination_codes: Location,
    #[serde(default)]
    city_name: Option<String>,
    #[serde(default)]
    images: Option<Images>,
    #[serde(default)]
    coordinates: Option<Coordinates>,
    #[serde(default)]
    pub country_code: String,
    #[serde(default)]
    unknown5: Option<bool>,
    #[serde(default)]
    pub country_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Images {
    unknown0: String,
    city_name: Option<String>,
    #[serde(default)]
    city_images_links: Option<Vec<Vec<String>>>,
    #[serde(default)]
    description_short: Option<String>,
    #[serde(default)]
    description_long: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Coordinates {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct LimitedFlightResultContainer {
    // [12, None, None, None, None, ['Limited flight results', 'Google Flights has limited flight results on this route', 3]]
    unknown0: i32,
    unknown1: Option<i32>,
    unknown2: Option<i32>,
    unknown3: Option<i32>,
    unknown4: Option<i32>,
    unknown5: LimitedFlightResults,
}

#[derive(Debug, Deserialize, Serialize)]
struct LimitedFlightResults {
    // ['Limited flight results', 'Google Flights has limited flight results on this route', 3]
    short: String,
    long: String,
    unknown3: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Weird10 {
    cities: Vec<CityImages>,
    #[serde(default)]
    cities1: Option<Vec<CityImages>>,
    #[serde(default)]
    cities3: Option<VisitedLocation>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VisitedLocation {
    airline_code_inbound: String,
    company_name_inbound: Vec<String>,
    flight_in_and_out_info: Vec<FlightInfo>,
    origin_airport: String,
    departure_date: Date,
    departure_hour: Hour,
    destination_airport: String,
    arrival_date: Date,
    arrival_hour: Hour,
    total_time_minutes: i32,
    unknown10: Option<i32>,
    unknown11: Option<String>,
    unknown12: bool,
    connecting_airports: Option<Value>,
    unknown14: Option<String>,
    unknown15: Option<String>,
    unknown16: Option<Vec<String>>,
    unknown17: String,
    unknown18: OtherStruct,
    unknown19: i32,
    #[serde(default)]
    unknown20: Option<String>,
    #[serde(default)]
    unknown21: Option<String>,
    #[serde(default)]
    unknown22: Option<Emissions>,
    #[serde(default)]
    unknown23: Option<Vec<i32>>,
    #[serde(default)]
    unknown24: Option<Vec<CostumerSupport>>,
    #[serde(default)]
    unknown25: Option<String>,
    #[serde(default)]
    unknown26: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CityImagesStruct {
    // 0 is not vec CityImages...
    cities0: Weird10,
    #[serde(default)]
    cities1: Option<OtherCityStruct>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OtherCityStruct {
    optional_images: Option<Vec<CityImages>>,
    images: Vec<CityImages>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FlightResponseContainer {
    pub responses: Vec<RawResponse>,
}

impl FlightResponseContainer {
    pub fn new(responses: Vec<RawResponse>) -> Self {
        Self { responses }
    }
    pub fn get_images_coordinates(&self) -> Vec<(&Coordinates, &Location)> {
        self.responses
            .iter()
            .flat_map(|f| f.get_images_coordinates())
            .collect()
    }

    pub fn get_all_images(&self) -> Vec<&CityImages> {
        self.responses
            .iter()
            .flat_map(|f| f.get_all_images())
            .collect()
    }
    pub fn get_usual_price_bound(&self) -> Option<i32> {
        let mut res: Vec<i32> = self
            .responses
            .iter()
            .flat_map(|f| f.get_usual_price_bound())
            .collect();
        res.sort();
        res.into_iter().next()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawResponse {
    unknown0: Unknown0,
    city_images: CityImagesStruct,
    pub best_flights: Option<ItineraryContainerList>, //No stop flights?
    pub other_flights: Option<ItineraryContainerList>,
    train_travels: Option<ItineraryContainerList>,
    pub price_graph: Option<PriceGraph>,
    travel_cheaper_different_date: Option<Vec<CheaperTravelDifferentDatesContainer>>,
    alliance_mappings: Option<AllianceMappings>,
    unknown8: Option<String>,
    unknown9: Option<String>,
    unknown10: Option<String>,
    baggage_allowance_links: Option<Vec<CostumerSupport>>,
    unknown12: Option<Unknown12>,
    unknown13: Option<String>,
    unknown14: OtherStruct,
    unknown15: Option<Unknown15>,
    unknown16: Option<String>,
    connection_city_images: Option<Vec<CityImages>>,
    #[serde(default)]
    unknown18: Option<Unknown12>,
    #[serde(default)]
    unknown19: Option<String>,
    #[serde(default)]
    unknown20: Option<bool>,
    #[serde(default)]
    unknown21: Option<bool>,
    #[serde(default)]
    limited_results: Option<Vec<LimitedFlightResultContainer>>,
    #[serde(default)]
    unknown23: Option<String>,
    #[serde(default)]
    unknown24: Option<bool>,
    #[serde(default)]
    trip_cost: Option<Vec<TripCost>>,
    #[serde(default)]
    costumer_support_links: Option<Vec<CostumerSupport>>,
    #[serde(default)]
    unknown27: Option<String>,
    #[serde(default)]
    train_travel2: Option<TrainTravel2>,
    #[serde(default)]
    unknown29: Option<Value>,
    #[serde(default)]
    unknown30: Option<TripCostContainer>,
    #[serde(default)]
    unknown31: Option<bool>,
    #[serde(default)]
    unknown32: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TrainTravel2 {
    unknown0: Option<Vec<TrainSolution>>,
    unknown1: Option<i32>,
    unknown2: OtherStruct,
}

#[derive(Debug, Deserialize, Serialize)]
struct TrainSolution {
    unknown0: Option<Vec<i32>>,
    unknown1: i32,
    dep_date: Date,
    dep_hour: Hour,
    arr_date: Date,
    arr_hour: Hour,
    duration: i32,
    unknown7: Option<i32>,
    price: Option<Vec<TripCost>>,
    unknown9: Option<i32>,
    departing_station: String,
    arriving_station: String,
}

impl RawResponse {
    pub fn maybe_get_all_flights(&self) -> Option<Vec<ItineraryContainer>> {
        let mut all_itineraries: Vec<ItineraryContainer> = Vec::new();

        let options_1: Option<Vec<ItineraryContainer>> =
            self.best_flights.as_ref().map(|f| f.itinerary_list.clone());
        let options_2: Option<Vec<ItineraryContainer>> = self
            .other_flights
            .as_ref()
            .map(|f| f.itinerary_list.clone());

        for maybe_itinerary in [options_1, options_2].into_iter().flatten() {
            all_itineraries.extend(maybe_itinerary);
        }
        match all_itineraries.len() {
            0 => None,
            _ => Some(all_itineraries),
        }
    }
    fn get_all_images(&self) -> Vec<&CityImages> {
        let cities: Vec<&CityImages> = self.city_images.cities0.cities.iter().collect();
        let connection_images: Vec<&CityImages> =
            self.connection_city_images.iter().flatten().collect();

        let images_1: Option<Vec<&CityImages>> = self
            .city_images
            .cities0
            .cities1
            .as_ref()
            .map(|f| f.iter().collect());
        let images_2: Option<Vec<&CityImages>> = self
            .city_images
            .cities1
            .as_ref()
            .map(|f| f.images.iter().collect());
        let images_3: Option<Vec<&CityImages>> = self
            .city_images
            .cities1
            .as_ref()
            .and_then(|f| f.optional_images.as_ref())
            .map(|f| f.iter().collect());

        let mut all_images: Vec<&CityImages> = Vec::new();

        all_images.extend(cities);
        all_images.extend(connection_images);
        for maybe_image in [images_1, images_2, images_3].into_iter().flatten() {
            all_images.extend(maybe_image);
        }
        all_images
    }

    fn get_images_coordinates(&self) -> Vec<(&Coordinates, &Location)> {
        let all_images = self.get_all_images();

        let coordinates: Vec<(&Coordinates, &Location)> = all_images
            .into_iter()
            .flat_map(|f| f.coordinates.as_ref().map(|c| (c, &f.destination_codes)))
            .collect();
        coordinates
    }

    fn get_usual_price_bound(&self) -> Option<i32> {
        self.price_graph
            .as_ref()
            .map(|f| f.usual_price_low_bound.price)
    }
}

pub fn create_raw_response_vec(raw_inputs: String) -> Result<FlightResponseContainer> {
    let outer: Vec<RawResponseContainerVec> = decode_outer_object(raw_inputs.as_ref())?;
    let inner_objects: Vec<String> = outer
        .into_iter()
        .flat_map(|f| f.resp)
        .filter_map(|f| f.payload)
        .collect();
    let inner: Vec<RawResponse> = inner_objects
        .into_iter()
        .map(|f| decode_inner_object(&f))
        .filter_map(|f| f.ok())
        .collect();
    let response = FlightResponseContainer::new(inner);
    Ok(response)
}

impl TryFrom<&str> for RawResponse {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let outer: Vec<RawResponseContainerVec> = decode_outer_object(value)?;
        let inner_object = outer
            .first()
            .ok_or_else(|| anyhow!("Malformed data!"))?
            .resp
            .first()
            .ok_or_else(|| anyhow!("Malformed data!"))?
            .payload
            .as_ref()
            .ok_or_else(|| anyhow!("Malformed data!"))?;
        decode_inner_object(inner_object)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RawResponseContainer {
    unknown0: String,
    unknown1: Option<i32>,
    pub payload: Option<String>,
    #[serde(default)]
    unknown3: Option<String>,
    #[serde(default)]
    unknown4: Option<String>,
    #[serde(default)]
    unknown5: Option<ErrorContainer>,
}

impl GetOuterErrorMessages for RawResponseContainer {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        match &self.unknown5 {
            Some(ErrorContainer::Error(e)) => e.get_error_messages(),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum ErrorContainer {
    Success(Vec<Option<i32>>),
    Error(ErrorFromBackend),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ErrorFromBackend {
    // [3,null,[["type.googleapis.com/travel.frontend.flights.ErrorResponse",[[null,null,0,"fC_PZeTFFMn91PIPw_uwsA4"],0]]]]]]
    unknown0: Option<i32>,
    unknown1: Option<String>,
    error_container: Option<Vec<ErrorSpecific>>,
}

impl GetOuterErrorMessages for ErrorFromBackend {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        let error_specific_vec: Vec<ErrorSpecific> = self.error_container.as_ref()?.to_vec();
        let messages: Vec<String> = error_specific_vec
            .iter()
            .filter_map(|f| f.error_message.as_ref())
            .map(|f| f.to_string())
            .collect();

        match messages.len() {
            0 => None,
            _ => Some(messages),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ErrorSpecific {
    // ["type.googleapis.com/travel.frontend.flights.ErrorResponse",[[null,null,0,"fC_PZeTFFMn91PIPw_uwsA4"],0]]
    error_message: Option<String>,
    garbage_data: Option<GarbageData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct GarbageData {
    // [[null,null,0,"fC_PZeTFFMn91PIPw_uwsA4"],0]
    garbage: Option<Value>,
    garbage_data: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct RawResponseContainerVec {
    pub resp: Vec<RawResponseContainer>,
}

impl GetOuterErrorMessages for RawResponseContainerVec {
    fn get_error_messages(&self) -> Option<Vec<String>> {
        let messages: Vec<String> = self
            .resp
            .iter()
            .filter_map(|f| f.get_error_messages())
            .flatten()
            .collect();
        match messages.len() {
            0 => None,
            _ => Some(messages),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_parse_airline_json() {
        let json_str = r#"["LX","1628",null,"SWISS"]"#;

        let result: Result<AirplaneInfo, serde_json::Error> = serde_json::from_str(json_str);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_flight_info_json() {
        let json_str = r#"[null,null,"Helvetic","ZRH","Zurich Airport","Milan Malpensa Airport","MXP",null,[13,10],null,[14,5],55,[null,null,null,null,null,true],2,"74 cm",null,1,"Embraer 195 E2",[null,true],false,[2024,1,27],[2024,1,27],["LX","1628",null,"SWISS"],null,null,1,null,null,null,null,"74 centimetres",37467]"#;
        let result: Result<FlightInfo, serde_json::Error> = serde_json::from_str(json_str);
        // println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_weird_thing() {
        let mystr = r#"["LX", ["SWISS"], [[null, null, null, "LUX", "Luxembourg Airport", "Zurich Airport", "ZRH", null, [10, 50], null, [11, 55], 65, [], 1, "76 cm", null, 1, "Airbus A220-100 Passenger", null, false, [2024, 1, 27], [2024, 1, 27], ["LX", "751", null, "SWISS"], null, null, 1, null, null, null, null, "76 centimetres", 40497], [null, null, "Helvetic", "ZRH", "Zurich Airport", "Milan Malpensa Airport", "MXP", null, [13, 10], null, [14, 5], 55, [null, null, null, null, null, true], 2, "74 cm", null, 1, "Embraer 195 E2", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["LX", "1628", null, "SWISS"], null, null, 1, null, null, null, null, "74 centimetres", 37467]], "LUX", [2024, 1, 27], [10, 50], "MXP", [2024, 1, 27], [14, 5], 195, null, null, false, [[75, "ZRH", "ZRH", null, "Zurich Airport", "Z\u00c3\u00bcrich", "Zurich Airport", "Z\u00c3\u00bcrich"]], null, null, null, "G3nUPe", [[1705070296848121, 139803069, 858572], null, null, null, null, [[2]]], 1, null, null, [null, null, 1, -9, null, true, true, 78000, 86000, null, 119000, 1, false], [1], [["LX", "SWISS", "https://www.swiss.com/gb/en/prepare/special-care"]]]"#;
        // let binding = mystr.replace(r#"\"#, "");
        // println!("{}",binding);
        // let otherstr:&str = binding.as_str();

        let result: Result<Itinerary, serde_json::Error> = serde_json::from_str(mystr);
        // println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_other_weird_thing() {
        let mystr = r#"[["LX", ["SWISS"], [[null, null, null, "LUX", "Luxembourg Airport", "Zurich Airport", "ZRH", null, [10, 50], null, [11, 55], 65, [], 1, "76 cm", null, 1, "Airbus A220-100 Passenger", null, false, [2024, 1, 27], [2024, 1, 27], ["LX", "751", null, "SWISS"], null, null, 1, null, null, null, null, "76 centimetres", 40497], [null, null, "Helvetic", "ZRH", "Zurich Airport", "Milan Malpensa Airport", "MXP", null, [13, 10], null, [14, 5], 55, [null, null, null, null, null, true], 2, "74 cm", null, 1, "Embraer 195 E2", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["LX", "1628", null, "SWISS"], null, null, 1, null, null, null, null, "74 centimetres", 37467]], "LUX", [2024, 1, 27], [10, 50], "MXP", [2024, 1, 27], [14, 5], 195, null, null, false, [[75, "ZRH", "ZRH", null, "Zurich Airport", "Z\u00c3\u00bcrich", "Zurich Airport", "Z\u00c3\u00bcrich"]], null, null, null, "G3nUPe", [[1705070296848121, 139803069, 858572], null, null, null, null, [[2]]], 1, null, null, [null, null, 1, -9, null, true, true, 78000, 86000, null, 119000, 1, false], [1], [["LX", "SWISS", "https://www.swiss.com/gb/en/prepare/special-care"]]], [[null, 138], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEgxMWDc1MXxMWDE2MjgaCgihaxACGgNFVVI4HHDDdQ=="], null, true, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCKFrIs4BCrgBClkKA0xVWBIZMjAyNC0wMS0yN1QxMDo1MDowMCswMTowMBoDWlJIIhkyMDI0LTAxLTI3VDExOjU1OjAwKzAxOjAwKgJMWDIDNzUxOgJMWEIDNzUxSAFSAzIyMQpbCgNaUkgSGTIwMjQtMDEtMjdUMTM6MTA6MDArMDE6MDAaA01YUCIZMjAyNC0wMS0yN1QxNDowNTowMCswMTowMCoCTFgyBDE2Mjg6AkxYQgQxNjI4SAFSAzI5NRIECAMQARgBKAAyBwoFU1dJU1M\\u003d\"]", [[1]], false]"#;

        let result: Result<ItineraryContainer, serde_json::Error> = serde_json::from_str(mystr);
        // println!("{:?}", result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_itinerary_list() {
        let mystr = r#"[[["LX", ["SWISS"], [[null, null, null, "LUX", "Luxembourg Airport", "Zurich Airport", "ZRH", null, [10, 50], null, [11, 55], 65, [], 1, "76 cm", null, 1, "Airbus A220-100 Passenger", null, false, [2024, 1, 27], [2024, 1, 27], ["LX", "751", null, "SWISS"], null, null, 1, null, null, null, null, "76 centimetres", 40497], [null, null, "Helvetic", "ZRH", "Zurich Airport", "Milan Malpensa Airport", "MXP", null, [13, 10], null, [14, 5], 55, [null, null, null, null, null, true], 2, "74 cm", null, 1, "Embraer 195 E2", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["LX", "1628", null, "SWISS"], null, null, 1, null, null, null, null, "74 centimetres", 37467]], "LUX", [2024, 1, 27], [10, 50], "MXP", [2024, 1, 27], [14, 5], 195, null, null, false, [[75, "ZRH", "ZRH", null, "Zurich Airport", "Z\u00c3\u00bcrich", "Zurich Airport", "Z\u00c3\u00bcrich"]], null, null, null, "G3nUPe", [[1705070296848121, 139803069, 858572], null, null, null, null, [[2]]], 1, null, null, [null, null, 1, -9, null, true, true, 78000, 86000, null, 119000, 1, false], [1], [["LX", "SWISS", "https://www.swiss.com/gb/en/prepare/special-care"]]], [[null, 138], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEgxMWDc1MXxMWDE2MjgaCgihaxACGgNFVVI4HHDDdQ=="], null, true, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCKFrIs4BCrgBClkKA0xVWBIZMjAyNC0wMS0yN1QxMDo1MDowMCswMTowMBoDWlJIIhkyMDI0LTAxLTI3VDExOjU1OjAwKzAxOjAwKgJMWDIDNzUxOgJMWEIDNzUxSAFSAzIyMQpbCgNaUkgSGTIwMjQtMDEtMjdUMTM6MTA6MDArMDE6MDAaA01YUCIZMjAyNC0wMS0yN1QxNDowNTowMCswMTowMCoCTFgyBDE2Mjg6AkxYQgQxNjI4SAFSAzI5NRIECAMQARgBKAAyBwoFU1dJU1M\\u003d\"]", [[1]], false], [["multi", ["Lufthansa", "Air Dolomiti"], [[null, null, "Lufthansa CityLine", "LUX", "Luxembourg Airport", "Munich International Airport", "MUC", null, [9, 40], null, [10, 45], 65, [], 1, "79 cm", null, 1, "Canadair RJ 900", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "2317", null, "Lufthansa"], null, null, 1, null, null, null, null, "79 centimetres", 63873], [null, null, null, "MUC", "Munich International Airport", "Milan Malpensa Airport", "MXP", null, [11, 30], null, [12, 35], 65, [], 1, "79 cm", [["LH", "9448", null, "Lufthansa"]], 1, "Embraer 195", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["EN", "8274", null, "Air Dolomiti"], null, null, 1, null, null, null, null, "79 centimetres", 55785]], "LUX", [2024, 1, 27], [9, 40], "MXP", [2024, 1, 27], [12, 35], 175, null, null, false, [[45, "MUC", "MUC", null, "Munich International Airport", "Munich", "Munich International Airport", "Munich"]], null, null, null, "zd8P7d", [[1705070296848121, 139803069, 858572], null, null, null, null, [[3]]], 1, null, null, [null, null, 3, 40, null, true, true, 120000, 86000, null, 119000, 2, false], [1], [["LH", "Lufthansa", "https://www.lufthansa.com/gb/en/travelling-with-special-requirements"], ["EN", "Air Dolomiti", "https://www.airdolomiti.eu/assistance"]]], [[null, 145], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEg1MSDIzMTd8RU44Mjc0GgoIoXEQAhoDRVVSOBxwjHw="], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCKFxIuIBCroBClsKA0xVWBIZMjAyNC0wMS0yN1QwOTo0MDowMCswMTowMBoDTVVDIhkyMDI0LTAxLTI3VDEwOjQ1OjAwKzAxOjAwKgJMSDIEMjMxNzoCTEhCBDIzMTdIAVIDQ1I5ClsKA01VQxIZMjAyNC0wMS0yN1QxMTozMDowMCswMTowMBoDTVhQIhkyMDI0LTAxLTI3VDEyOjM1OjAwKzAxOjAwKgJFTjIEODI3NDoCTEhCBDk0NDhIAVIDRTk1EgQIAxABGAEoADIZCglMdWZ0aGFuc2EKDEFpciBEb2xvbWl0aQ\\u003d\\u003d\"]", [[2]], false], [["KL", ["KLM"], [[null, null, "German Airways", "LUX", "Luxembourg Airport", "Amsterdam Airport Schiphol", "AMS", null, [14, 45], null, [16], 75, [null, null, null, null, null, true], 1, "79 cm", null, 1, "Embraer 190", null, false, [2024, 1, 27], [2024, 1, 27], ["KL", "1742", null, "KLM"], null, null, 1, null, null, null, null, "79 centimetres", 57027], [null, null, "KLM Cityhopper", "AMS", "Amsterdam Airport Schiphol", "Linate Airport", "LIN", null, [16, 55], null, [18, 35], 100, [], 2, "74 cm", null, 1, "Embraer 175", null, false, [2024, 1, 27], [2024, 1, 27], ["KL", "1621", null, "KLM"], null, null, 1, null, null, null, null, "74 centimetres", 91358]], "LUX", [2024, 1, 27], [14, 45], "LIN", [2024, 1, 27], [18, 35], 230, null, null, false, [[55, "AMS", "AMS", null, "Amsterdam Airport Schiphol", "Amsterdam", "Amsterdam Airport Schiphol", "Amsterdam"]], null, null, null, "goZ5db", [[1705070296848121, 139803069, 858572], null, null, null, null, [[4]]], 1, null, null, [null, null, 3, 72, null, true, true, 148000, 86000, null, 119000, 3, false], [1], [["KL", "KLM", "https://www.klm.co.uk/information/assistance-health"]]], [[null, 154], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEg1LTDE3NDJ8S0wxNjIxGgoI4ncQAhoDRVVSOBxwnYMB"], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCOJ3Is4BCroBClsKA0xVWBIZMjAyNC0wMS0yN1QxNDo0NTowMCswMTowMBoDQU1TIhkyMDI0LTAxLTI3VDE2OjAwOjAwKzAxOjAwKgJLTDIEMTc0MjoCS0xCBDE3NDJIAVIDRTkwClsKA0FNUxIZMjAyNC0wMS0yN1QxNjo1NTowMCswMTowMBoDTElOIhkyMDI0LTAxLTI3VDE4OjM1OjAwKzAxOjAwKgJLTDIEMTYyMToCS0xCBDE2MjFIAVIDRTdXEgQIAxABGAEoADIFCgNLTE0\\u003d\"]", [[2]], false], [["LH", ["Lufthansa"], [[null, null, "Lufthansa CityLine", "LUX", "Luxembourg Airport", "Frankfurt Airport", "FRA", null, [6, 35], null, [7, 25], 50, [], 3, "81 cm", null, 1, "Embraer 190", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "399", null, "Lufthansa"], null, null, 1, null, null, null, null, "81 centimetres", 46528], [null, null, null, "FRA", "Frankfurt Airport", "Milan Malpensa Airport", "MXP", null, [9, 10], null, [10, 20], 70, [], 1, "76 cm", null, 1, "Airbus A320", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "246", null, "Lufthansa"], null, null, 1, null, null, null, null, "76 centimetres", 55017]], "LUX", [2024, 1, 27], [6, 35], "MXP", [2024, 1, 27], [10, 20], 225, null, null, false, [[105, "FRA", "FRA", null, "Frankfurt Airport", "Frankfurt", "Frankfurt Airport", "Frankfurt"]], null, null, null, "D2ou8e", [[1705070296848121, 139803069, 858572], null, null, null, null, [[5]]], 1, null, null, [null, null, 3, 19, null, true, true, 102000, 86000, null, 119000, 1, false], [1], [["LH", "Lufthansa", "https://www.lufthansa.com/gb/en/travelling-with-special-requirements"]]], [[null, 159], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEgtMSDM5OXxMSDI0NhoKCIZ8EAIaA0VVUjgccPWHAQ=="], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCIZ8ItABCrYBClkKA0xVWBIZMjAyNC0wMS0yN1QwNjozNTowMCswMTowMBoDRlJBIhkyMDI0LTAxLTI3VDA3OjI1OjAwKzAxOjAwKgJMSDIDMzk5OgJMSEIDMzk5SAFSA0U5MApZCgNGUkESGTIwMjQtMDEtMjdUMDk6MTA6MDArMDE6MDAaA01YUCIZMjAyNC0wMS0yN1QxMDoyMDowMCswMTowMCoCTEgyAzI0NjoCTEhCAzI0NkgBUgMzMjASBAgDEAEYASgAMgsKCUx1ZnRoYW5zYQ\\u003d\\u003d\"]", [[2]], false], [["LG", ["Luxair"], [[null, null, null, "LUX", "Luxembourg Airport", "Milan Malpensa Airport", "MXP", null, [11, 10], null, [12, 25], 75, [], 1, "76 cm", [["AZ", "7879", null, "ITA"]], 1, "De Havilland-Bombardier Dash-8", null, false, [2024, 1, 27], [2024, 1, 27], ["LG", "6993", null, "Luxair"], null, null, 1, null, null, null, null, "76 centimetres", 35968]], "LUX", [2024, 1, 27], [11, 10], "MXP", [2024, 1, 27], [12, 25], 75, null, null, false, null, null, null, ["ITA"], "VDOwRb", [[1705070296848121, 139803069, 858572], null, null, null, null, [[6]]], 1, null, null, [null, null, 1, -58, null, true, true, 36000, 86000, [true], 119000, 1, false], [1], [["LG", "Luxair", "https://www.luxair.lu/en/information/passenger-assistance"]]], [[null, 230], "CjRISnlhWXVsbHpfclVBSEhWcVFCRy0tLS0tLS0td2VicXIxMkFBQUFBR1doVHRnTTFpVHVBEgZMRzY5OTMaCwjVswEQAhoDRVVSOBxw7sQB"], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoECNWzASJ4Cl0KWwoDTFVYEhkyMDI0LTAxLTI3VDExOjEwOjAwKzAxOjAwGgNNWFAiGTIwMjQtMDEtMjdUMTI6MjU6MDArMDE6MDAqAkxHMgQ2OTkzOgJMR0IENjk5M0gBUgNESDQSBAgDEAEYASgAMg0KBkx1eGFpcgoDSVRB\"]", [[1]], false]]"#;

        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(mystr);
        let result: Result<Vec<ItineraryContainer>, _> = serde_path_to_error::deserialize(jd);
        // println!("{:?}", result);
        match result {
            Ok(_) => assert!(result.is_ok()),
            Err(err) => {
                let path = err.path().to_string();
                println!("{}", path);
                assert!(false)
            }
        }
    }

    #[test]
    fn test_itinerary_list_container() {
        let mystr = r#"[[[["LX", ["SWISS"], [[null, null, null, "LUX", "Luxembourg Airport", "Zurich Airport", "ZRH", null, [10, 50], null, [11, 55], 65, [], 1, "76 cm", null, 1, "Airbus A220-100 Passenger", null, false, [2024, 1, 27], [2024, 1, 27], ["LX", "751", null, "SWISS"], null, null, 1, null, null, null, null, "76 centimetres", 40497], [null, null, "Helvetic", "ZRH", "Zurich Airport", "Milan Malpensa Airport", "MXP", null, [13, 10], null, [14, 5], 55, [null, null, null, null, null, true], 2, "74 cm", null, 1, "Embraer 195 E2", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["LX", "1628", null, "SWISS"], null, null, 1, null, null, null, null, "74 centimetres", 37467]], "LUX", [2024, 1, 27], [10, 50], "MXP", [2024, 1, 27], [14, 5], 195, null, null, false, [[75, "ZRH", "ZRH", null, "Zurich Airport", "Z\u00c3\u00bcrich", "Zurich Airport", "Z\u00c3\u00bcrich"]], null, null, null, "G3nUPe", [[1705063796213762, 139803069, 858572], null, null, null, null, [[2]]], 1, null, null, [null, null, 1, -9, null, true, true, 78000, 86000, null, 119000, 1, false], [1], [["LX", "SWISS", "https://www.swiss.com/gb/en/prepare/special-care"]]], [[null, 138], "CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEgxMWDc1MXxMWDE2MjgaCgihaxACGgNFVVI4HHCxdQ=="], null, true, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCKFrIs4BCrgBClkKA0xVWBIZMjAyNC0wMS0yN1QxMDo1MDowMCswMTowMBoDWlJIIhkyMDI0LTAxLTI3VDExOjU1OjAwKzAxOjAwKgJMWDIDNzUxOgJMWEIDNzUxSAFSAzIyMQpbCgNaUkgSGTIwMjQtMDEtMjdUMTM6MTA6MDArMDE6MDAaA01YUCIZMjAyNC0wMS0yN1QxNDowNTowMCswMTowMCoCTFgyBDE2Mjg6AkxYQgQxNjI4SAFSAzI5NRIECAMQARgBKAAyBwoFU1dJU1M\\u003d\"]", [[1]], false], [["multi", ["Lufthansa", "Air Dolomiti"], [[null, null, "Lufthansa CityLine", "LUX", "Luxembourg Airport", "Munich International Airport", "MUC", null, [9, 40], null, [10, 45], 65, [], 1, "79 cm", null, 1, "Canadair RJ 900", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "2317", null, "Lufthansa"], null, null, 1, null, null, null, null, "79 centimetres", 63873], [null, null, null, "MUC", "Munich International Airport", "Milan Malpensa Airport", "MXP", null, [11, 30], null, [12, 35], 65, [], 1, "79 cm", [["LH", "9448", null, "Lufthansa"]], 1, "Embraer 195", [null, true], false, [2024, 1, 27], [2024, 1, 27], ["EN", "8274", null, "Air Dolomiti"], null, null, 1, null, null, null, null, "79 centimetres", 55785]], "LUX", [2024, 1, 27], [9, 40], "MXP", [2024, 1, 27], [12, 35], 175, null, null, false, [[45, "MUC", "MUC", null, "Munich International Airport", "Munich", "Munich International Airport", "Munich"]], null, null, null, "zd8P7d", [[1705063796213762, 139803069, 858572], null, null, null, null, [[3]]], 1, null, null, [null, null, 3, 40, null, true, true, 120000, 86000, null, 119000, 2, false], [1], [["LH", "Lufthansa", "https://www.lufthansa.com/gb/en/travelling-with-special-requirements"], ["EN", "Air Dolomiti", "https://www.airdolomiti.eu/assistance"]]], [[null, 145], "CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEg1MSDIzMTd8RU44Mjc0GgoIoXEQAhoDRVVSOBxw+ns="], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCKFxIuIBCroBClsKA0xVWBIZMjAyNC0wMS0yN1QwOTo0MDowMCswMTowMBoDTVVDIhkyMDI0LTAxLTI3VDEwOjQ1OjAwKzAxOjAwKgJMSDIEMjMxNzoCTEhCBDIzMTdIAVIDQ1I5ClsKA01VQxIZMjAyNC0wMS0yN1QxMTozMDowMCswMTowMBoDTVhQIhkyMDI0LTAxLTI3VDEyOjM1OjAwKzAxOjAwKgJFTjIEODI3NDoCTEhCBDk0NDhIAVIDRTk1EgQIAxABGAEoADIZCglMdWZ0aGFuc2EKDEFpciBEb2xvbWl0aQ\\u003d\\u003d\"]", [[2]], false], [["KL", ["KLM"], [[null, null, "German Airways", "LUX", "Luxembourg Airport", "Amsterdam Airport Schiphol", "AMS", null, [14, 45], null, [16], 75, [null, null, null, null, null, true], 1, "79 cm", null, 1, "Embraer 190", null, false, [2024, 1, 27], [2024, 1, 27], ["KL", "1742", null, "KLM"], null, null, 1, null, null, null, null, "79 centimetres", 57027], [null, null, "KLM Cityhopper", "AMS", "Amsterdam Airport Schiphol", "Linate Airport", "LIN", null, [16, 55], null, [18, 35], 100, [], 2, "74 cm", null, 1, "Embraer 175", null, false, [2024, 1, 27], [2024, 1, 27], ["KL", "1621", null, "KLM"], null, null, 1, null, null, null, null, "74 centimetres", 91358]], "LUX", [2024, 1, 27], [14, 45], "LIN", [2024, 1, 27], [18, 35], 230, null, null, false, [[55, "AMS", "AMS", null, "Amsterdam Airport Schiphol", "Amsterdam", "Amsterdam Airport Schiphol", "Amsterdam"]], null, null, null, "goZ5db", [[1705063796213762, 139803069, 858572], null, null, null, null, [[4]]], 1, null, null, [null, null, 3, 72, null, true, true, 148000, 86000, null, 119000, 3, false], [1], [["KL", "KLM", "https://www.klm.co.uk/information/assistance-health"]]], [[null, 154], "CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEg1LTDE3NDJ8S0wxNjIxGgoI4ncQAhoDRVVSOBxwiYMB"], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCOJ3Is4BCroBClsKA0xVWBIZMjAyNC0wMS0yN1QxNDo0NTowMCswMTowMBoDQU1TIhkyMDI0LTAxLTI3VDE2OjAwOjAwKzAxOjAwKgJLTDIEMTc0MjoCS0xCBDE3NDJIAVIDRTkwClsKA0FNUxIZMjAyNC0wMS0yN1QxNjo1NTowMCswMTowMBoDTElOIhkyMDI0LTAxLTI3VDE4OjM1OjAwKzAxOjAwKgJLTDIEMTYyMToCS0xCBDE2MjFIAVIDRTdXEgQIAxABGAEoADIFCgNLTE0\\u003d\"]", [[2]], false], [["LH", ["Lufthansa"], [[null, null, "Lufthansa CityLine", "LUX", "Luxembourg Airport", "Frankfurt Airport", "FRA", null, [6, 35], null, [7, 25], 50, [], 3, "81 cm", null, 1, "Embraer 190", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "399", null, "Lufthansa"], null, null, 1, null, null, null, null, "81 centimetres", 46528], [null, null, null, "FRA", "Frankfurt Airport", "Milan Malpensa Airport", "MXP", null, [9, 10], null, [10, 20], 70, [], 1, "76 cm", null, 1, "Airbus A320", null, false, [2024, 1, 27], [2024, 1, 27], ["LH", "246", null, "Lufthansa"], null, null, 1, null, null, null, null, "76 centimetres", 55017]], "LUX", [2024, 1, 27], [6, 35], "MXP", [2024, 1, 27], [10, 20], 225, null, null, false, [[105, "FRA", "FRA", null, "Frankfurt Airport", "Frankfurt", "Frankfurt Airport", "Frankfurt"]], null, null, null, "D2ou8e", [[1705063796213762, 139803069, 858572], null, null, null, null, [[5]]], 1, null, null, [null, null, 3, 19, null, true, true, 102000, 86000, null, 119000, 1, false], [1], [["LH", "Lufthansa", "https://www.lufthansa.com/gb/en/travelling-with-special-requirements"]]], [[null, 159], "CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEgtMSDM5OXxMSDI0NhoKCIZ8EAIaA0VVUjgccOGHAQ=="], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoDCIZ8ItABCrYBClkKA0xVWBIZMjAyNC0wMS0yN1QwNjozNTowMCswMTowMBoDRlJBIhkyMDI0LTAxLTI3VDA3OjI1OjAwKzAxOjAwKgJMSDIDMzk5OgJMSEIDMzk5SAFSA0U5MApZCgNGUkESGTIwMjQtMDEtMjdUMDk6MTA6MDArMDE6MDAaA01YUCIZMjAyNC0wMS0yN1QxMDoyMDowMCswMTowMCoCTEgyAzI0NjoCTEhCAzI0NkgBUgMzMjASBAgDEAEYASgAMgsKCUx1ZnRoYW5zYQ\\u003d\\u003d\"]", [[2]], false], [["LG", ["Luxair"], [[null, null, null, "LUX", "Luxembourg Airport", "Milan Malpensa Airport", "MXP", null, [11, 10], null, [12, 25], 75, [], 1, "76 cm", [["AZ", "7879", null, "ITA"]], 1, "De Havilland-Bombardier Dash-8", null, false, [2024, 1, 27], [2024, 1, 27], ["LG", "6993", null, "Luxair"], null, null, 1, null, null, null, null, "76 centimetres", 35968]], "LUX", [2024, 1, 27], [11, 10], "MXP", [2024, 1, 27], [12, 25], 75, null, null, false, null, null, null, ["ITA"], "VDOwRb", [[1705063796213762, 139803069, 858572], null, null, null, null, [[6]]], 1, null, null, [null, null, 1, -58, null, true, true, 36000, 86000, [true], 119000, 1, false], [1], [["LG", "Luxair", "https://www.luxair.lu/en/information/passenger-assistance"]]], [[null, 230], "CjRIX0VzeG9hMURhNElBRGVpM2dCRy0tLS0tLS0tLXdmZG4yMEFBQUFBR1doTlhRRFc1SE9BEgZMRzY5OTMaCwjVswEQAhoDRVVSOBxw0MQB"], null, false, [], [false, false, false], false, [], "[\"CAISA0VVUhoECNWzASJ4Cl0KWwoDTFVYEhkyMDI0LTAxLTI3VDExOjEwOjAwKzAxOjAwGgNNWFAiGTIwMjQtMDEtMjdUMTI6MjU6MDArMDE6MDAqAkxHMgQ2OTkzOgJMR0IENjk5M0gBUgNESDQSBAgDEAEYASgAMg0KBkx1eGFpcgoDSVRB\"]", [[1]], false]], null, false, false, [1]]"#;

        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(mystr);
        let result: Result<ItineraryContainerList, _> = serde_path_to_error::deserialize(jd);
        // println!("{:?}", result);
        match result {
            Ok(_) => assert!(result.is_ok()),
            Err(err) => {
                let path = err.path().to_string();
                println!("{}", path);
                assert!(false)
            }
        }
    }

    #[test]
    fn test_raw_response_all() {
        let mystr =
            fs::read_to_string("test_files/raw_gflights.response").expect("Cannot read from file");

        let raw_resp: RawResponseContainerVec =
            serde_json::from_str(&mystr).expect("Error in parsing");
        let inner_obj = &raw_resp.resp[0].payload.as_ref().unwrap();
        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(inner_obj);
        let result: Result<RawResponse, _> = serde_path_to_error::deserialize(jd);
        // println!("{:?}", result);
        match result {
            Ok(_) => assert!(result.is_ok()),
            Err(err) => {
                println!("{}", err);
                let path = err.path().to_string();
                println!("{}", path);
                assert!(false)
            }
        }
    }

    #[test]
    fn test_flight_warnings() {
        let raw_resp = r#"[2, "Mytrip", "https://lu.mytrip.com/rf/self-transfer"]"#;

        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(raw_resp);
        let result: Result<ItineraryWarnings, _> = serde_path_to_error::deserialize(jd);
        println!("{:?}", result);
        match result {
            Ok(_) => assert!(result.is_ok()),
            Err(err) => {
                let path = err.path().to_string();
                println!("{}", path);
                assert!(false)
            }
        }
    }

    #[test]
    fn test_tokyo_response() {
        let datafiles = [
            "test_files/lux_tokyo_oneway.txt",
            "test_files/lux_milan_oneway.txt",
            "test_files/lux_dubai_oneway.txt",
            "test_files/flights_new_test.txt",
            "test_files/response_non_uniform_city_images.txt",
            "test_files/raw.response",
        ];
        for itinerary in datafiles.iter() {
            let mystr = fs::read_to_string(itinerary).expect("Cannot read from file");

            let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
                &mut serde_json::Deserializer::from_str(&mystr);
            let result: Result<RawResponse, _> = serde_path_to_error::deserialize(jd);
            // println!("{:?}", result);
            match result {
                Ok(_) => assert!(result.is_ok()),
                Err(err) => {
                    println!("{}", err);
                    let path = err.path().to_string();
                    println!("Path to error {}, datafile {}", path, itinerary);
                    assert!(false)
                }
            }
        }
    }
    #[test]
    fn test_multi_line_response() {
        let datafiles = "test_files/raw_multiline.txt";
        let mystr = fs::read_to_string(datafiles).expect("Cannot read from file");
        let additionals = mystr
            .lines()
            .skip(3)
            .step_by(2)
            .filter(|f| f.starts_with(r#"[["wrb.fr""#))
            .max_by_key(|line| line.len())
            .unwrap_or_default();
        let raw_resp: RawResponseContainerVec =
            serde_json::from_str(additionals).expect("Error in parsing");
        let inner_obj = &raw_resp.resp[0].payload.as_ref().unwrap();
        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(inner_obj);
        let result: Result<RawResponse, _> = serde_path_to_error::deserialize(jd);

        assert!(result.is_ok())
    }

    #[test]
    fn test_city_images() {
        let mystr = r#"[["/m/047vq3s",4],"Zanzibar",["/m/047vq3s","Zanzibar",[["https://encrypted-tbn0.gstatic.com/licensed-image?q\u003dtbn:ANd9GcS6jLYJnuoL4WiQ4PgYxvWM9x9JbTkFoBCY-uGGEba9qzJhnfJXsRhnmD1xqYTS3Tsjwb0goZ44qDuxY_D-6rFIqd15qFufOf2xFbRpU-Q"],["https://encrypted-tbn0.gstatic.com/licensed-image?q\u003dtbn:ANd9GcQiFVWU2w2bR4o8GkCUNGu6Y3iQkJ6fEAAbfsTu2N3_d0R57Rd83MvuAg4v4sj1YndR6QiSz3_SWmK53jvM1jqAtngulIHgxtYFLEC_hdM"]]]]"#;

        let jd: &mut serde_json::Deserializer<serde_json::de::StrRead<'_>> =
            &mut serde_json::Deserializer::from_str(mystr);
        let result: Result<CityImages, _> = serde_path_to_error::deserialize(jd);
        // println!("{:?}", result);
        match result {
            Ok(_) => assert!(result.is_ok()),
            Err(err) => {
                println!("{}", err);
                let path = err.path().to_string();
                println!("Path to error {}", path);
                assert!(false)
            }
        }
    }

    #[test]
    fn it_works_check_low_price_is_some() -> Result<()> {
        let my_string =
            fs::read_to_string("test_files/low_price_in_second_line.txt").expect("error here");

        let outer: Vec<RawResponseContainerVec> = decode_outer_object(my_string.as_ref())?;

        let inner_objects: Vec<String> = outer
            .into_iter()
            .flat_map(|f| f.resp)
            .filter_map(|f| f.payload)
            .collect();

        let inner: Vec<RawResponse> = inner_objects
            .into_iter()
            .flat_map(|f| decode_inner_object(&f))
            .collect();

        let low_price_usual: Vec<Option<i32>> = inner
            .iter()
            .map(|f| {
                f.price_graph
                    .as_ref()
                    .map(|f| f.usual_price_low_bound.price)
            })
            .filter(|f| f.is_some())
            .collect();

        assert!(low_price_usual.first().unwrap().is_some());
        Ok(())
    }

    #[test]
    fn test_return_response() -> Result<()> {
        let datafiles = "test_files/response_with_first_fixed_full.txt";
        let mystr = fs::read_to_string(datafiles).expect("Cannot read from file");
        let result = create_raw_response_vec(mystr);

        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_hour_can_be_empty() {
        let hour_str = "{}".to_string();
        let hour = serde_json::from_str::<Hour>(&hour_str);
        assert!(hour.is_ok());
        let parsed = serde_json::to_string(&hour.unwrap()).unwrap();
        let res = r#"{"hour":null,"minute":0}"#.to_string();
        assert_eq!(parsed, res);
    }

    #[test]
    fn test_cheaper_travel_different_places_can_be_empty() {
        let cheaper_travel_str = "{}".to_string();
        let cheaper_travel =
            serde_json::from_str::<CheaperTravelDifferentPlaces>(&cheaper_travel_str);
        assert!(cheaper_travel.is_ok());
        let parsed = serde_json::to_string(&cheaper_travel.unwrap()).unwrap();
        let res = r#"{"dates":null,"airports":null}"#.to_string();
        assert_eq!(parsed, res);
    }

    #[test]
    fn test_test_return_response() -> Result<()> {
        let datafiles = [
            "test_files/error0.txt",
            "test_files/error1.txt",
            "test_files/with_28_elements.txt",
        ]
        .to_vec();

        for datafile in datafiles.iter() {
            let mystr = fs::read_to_string(datafile).expect("Cannot read from file");
            let other: Result<RawResponse, _> = decode_inner_object(&mystr);
            match other {
                Ok(_) => assert!(other.is_ok()),
                Err(err) => {
                    println!("datafile {:?}", datafile);
                    assert!(false)
                }
            }
        }
        Ok(())
    }
}
