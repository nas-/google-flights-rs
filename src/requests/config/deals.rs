//! Configuration and result types for the `GetFlightDealsStreaming` endpoint.

use crate::parsers::common::{Location, TravelClass, Travelers};
use crate::requests::config::Currency;
use chrono::NaiveDate;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for a `GetFlightDealsStreaming` request.
///
/// The endpoint returns discounted round-trip destinations from an origin. The
/// `outbound_date` / `return_date` pair acts as a **trip-length anchor**: the
/// backend returns deals of a similar length across many dates, not only that
/// exact pair.
///
/// All fields except `origin`, `outbound_date`, and `return_date` have sensible
/// defaults via [`Default`].
#[derive(Debug, Clone)]
pub struct DealConfig {
    /// Origin airport(s) or city. Airports use `PlaceType::Airport`; cities use
    /// `PlaceType::City`.
    pub origin: Vec<Location>,

    /// Outbound date (trip-length anchor).
    pub outbound_date: NaiveDate,

    /// Return date (trip-length anchor).
    pub return_date: NaiveDate,

    /// Restrict to non-stop flights only.
    pub nonstop: bool,

    /// Maximum one-way flight duration in minutes.
    pub max_duration_minutes: Option<u32>,

    /// Cabin class.
    pub travel_class: TravelClass,

    /// Traveller counts.
    pub travellers: Travelers,

    /// Currency for prices.
    pub currency: Currency,

    /// BCP-47 language subtag, e.g. `"en"`.
    pub language: String,

    /// ISO 3166-1 alpha-2 country code, e.g. `"GB"`.
    pub country: String,
}

impl Default for DealConfig {
    fn default() -> Self {
        Self {
            origin: Vec::new(),
            // Placeholder anchor; callers are expected to set real dates.
            outbound_date: NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            return_date: NaiveDate::from_ymd_opt(2025, 1, 4).expect("valid date"),
            nonstop: false,
            max_duration_minutes: None,
            travel_class: TravelClass::Economy,
            travellers: Travelers::default(),
            currency: Currency::default(),
            language: "en".to_string(),
            country: "GB".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// One discounted destination returned by `GetFlightDealsStreaming`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DealResult {
    /// Origin airport IATA code.
    pub origin_iata: String,
    /// Destination airport IATA code.
    pub destination_iata: String,
    /// Destination city name.
    pub destination_city: String,
    /// Destination country name.
    pub destination_country: String,
    /// Destination Google place MID (e.g. `"/m/04llb"`), if present.
    pub destination_mid: Option<String>,
    /// Outbound departure date.
    pub outbound_date: Option<NaiveDate>,
    /// Return date.
    pub return_date: Option<NaiveDate>,
    /// Deal price (round trip) in the requested currency.
    pub price: Option<i32>,
    /// Typical price for this route (the baseline the discount is measured against).
    pub typical_price: Option<i32>,
    /// Percentage below typical price (e.g. `68` = 68% off).
    pub discount_pct: Option<i32>,
    /// Total flight duration in minutes.
    pub duration_minutes: Option<u32>,
    /// Number of stops (0 = non-stop).
    pub stops: Option<u8>,
    /// Operating airline code (may be `"*"` for mixed carriers).
    pub airline_code: Option<String>,
    /// Operating airline name.
    pub airline_name: Option<String>,
    /// Cover image URL for the destination.
    pub image_url: Option<String>,
    /// Short highlight phrases for the destination.
    pub highlights: Vec<String>,
    /// One-line description of the destination.
    pub description: Option<String>,
    /// Ready-to-open Google Flights booking deep link (absolute URL).
    pub booking_url: Option<String>,
    /// Opaque booking token from the deal entry.
    pub booking_token: Option<String>,
}
