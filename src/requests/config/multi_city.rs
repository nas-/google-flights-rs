use anyhow::{anyhow, Result};
use chrono::NaiveDate;

use crate::parsers::common::{
    AirlineFilter, FlightTimes, Location, PlaceType, SortOrder, StopOptions, StopoverDuration,
    TotalDuration, TravelClass, Travelers,
};
use crate::protos::urls::{ItineraryUrl, Leg};
use crate::requests::api::ApiClient;

use super::builder::get_location_pub;
use super::currency::Currency;

/// Per-leg filter overrides for multi-city searches.
///
/// All fields default to "unrestricted" (same as `Config`'s defaults).
/// Obtain a default value via [`LegFilters::default()`] and override
/// only the fields you need.
#[derive(Debug, Clone)]
pub struct LegFilters {
    pub stop_options: StopOptions,
    pub airlines_include: Vec<AirlineFilter>,
    pub airlines_exclude: Vec<AirlineFilter>,
    /// IATA airport codes required as connection points.
    pub connecting_airports: Vec<String>,
    pub stopover_min: StopoverDuration,
    pub stopover_max: StopoverDuration,
    pub duration_max: TotalDuration,
    /// If `true`, only return flights with below-average CO₂ emissions.
    pub lower_emissions: bool,
    pub departing_times: FlightTimes,
}

impl Default for LegFilters {
    fn default() -> Self {
        Self {
            stop_options: StopOptions::All,
            airlines_include: Vec::new(),
            airlines_exclude: Vec::new(),
            connecting_airports: Vec::new(),
            stopover_min: StopoverDuration::default(),
            stopover_max: StopoverDuration::default(),
            duration_max: TotalDuration::default(),
            lower_emissions: false,
            departing_times: FlightTimes::default(),
        }
    }
}

/// A single leg in a multi-city itinerary.
#[derive(Debug, Clone)]
pub struct MultiCityLeg {
    /// One to seven origin airports / city identifiers.
    pub from: Vec<Location>,
    /// One to seven destination airports / city identifiers.
    pub to: Vec<Location>,
    pub date: NaiveDate,
    // Per-leg filters — default to "unrestricted".
    pub stop_options: StopOptions,
    pub airlines_include: Vec<AirlineFilter>,
    pub airlines_exclude: Vec<AirlineFilter>,
    /// IATA airport codes required as connection points.
    pub connecting_airports: Vec<String>,
    pub stopover_min: StopoverDuration,
    pub stopover_max: StopoverDuration,
    pub duration_max: TotalDuration,
    /// If `true`, only return flights with below-average CO₂ emissions.
    pub lower_emissions: bool,
    pub departing_times: FlightTimes,
}

/// Configuration for a multi-city (open-jaw) flight search.
///
/// Build with [`MultiCityConfig::builder()`], then pass to
/// [`ApiClient::request_multi_city_flights`].
///
/// Google Flights returns independent flight options per leg; the result is
/// `Vec<FlightResponseContainer>` — one container per leg.
#[derive(Debug, Clone)]
pub struct MultiCityConfig {
    /// Ordered list of legs (minimum 2).
    pub legs: Vec<MultiCityLeg>,
    pub travellers: Travelers,
    pub travel_class: TravelClass,
    pub sort_order: SortOrder,
    pub currency: Currency,
    /// Maximum total ticket price cap. `None` = no limit.
    pub max_price: Option<i32>,
    /// Baggage allowance `(carry_on_count, checked_count)`. `None` = no restriction.
    pub baggage: Option<(u8, u8)>,
    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`.
    pub language: String,
    /// ISO 3166-1 alpha-2 country code, e.g. `"GB"`, `"US"`.
    pub country: String,
}

impl MultiCityConfig {
    pub fn builder() -> MultiCityConfigBuilder {
        MultiCityConfigBuilder::default()
    }
}

/// Builder for [`MultiCityConfig`]. Obtain via [`MultiCityConfig::builder()`].
#[derive(Default)]
pub struct MultiCityConfigBuilder {
    legs: Vec<MultiCityLeg>,
    travellers: Travelers,
    travel_class: TravelClass,
    sort_order: SortOrder,
    currency: Option<Currency>,
    max_price: Option<i32>,
    baggage: Option<(u8, u8)>,
    language: String,
    country: String,
}

impl MultiCityConfigBuilder {
    /// Resolve `from` and `to` via the API and append a leg.
    pub async fn add_leg(
        mut self,
        from: &str,
        to: &str,
        date: NaiveDate,
        client: &ApiClient,
    ) -> Result<Self> {
        let from_loc = get_location_pub(from, client).await?;
        let to_loc = get_location_pub(to, client).await?;
        let filters = LegFilters::default();
        self.legs.push(MultiCityLeg {
            from: vec![from_loc],
            to: vec![to_loc],
            date,
            stop_options: filters.stop_options,
            airlines_include: filters.airlines_include,
            airlines_exclude: filters.airlines_exclude,
            connecting_airports: filters.connecting_airports,
            stopover_min: filters.stopover_min,
            stopover_max: filters.stopover_max,
            duration_max: filters.duration_max,
            lower_emissions: filters.lower_emissions,
            departing_times: filters.departing_times,
        });
        Ok(self)
    }

    /// Resolve `from` and `to` via the API and append a leg with per-leg filters.
    pub async fn add_leg_with_filters(
        mut self,
        from: &str,
        to: &str,
        date: NaiveDate,
        client: &ApiClient,
        filters: LegFilters,
    ) -> Result<Self> {
        let from_loc = get_location_pub(from, client).await?;
        let to_loc = get_location_pub(to, client).await?;
        self.legs.push(MultiCityLeg {
            from: vec![from_loc],
            to: vec![to_loc],
            date,
            stop_options: filters.stop_options,
            airlines_include: filters.airlines_include,
            airlines_exclude: filters.airlines_exclude,
            connecting_airports: filters.connecting_airports,
            stopover_min: filters.stopover_min,
            stopover_max: filters.stopover_max,
            duration_max: filters.duration_max,
            lower_emissions: filters.lower_emissions,
            departing_times: filters.departing_times,
        });
        Ok(self)
    }

    /// Append a leg using pre-resolved [`Location`] values (no network call).
    pub fn add_leg_locations(
        mut self,
        from: Vec<Location>,
        to: Vec<Location>,
        date: NaiveDate,
    ) -> Self {
        let filters = LegFilters::default();
        self.legs.push(MultiCityLeg {
            from,
            to,
            date,
            stop_options: filters.stop_options,
            airlines_include: filters.airlines_include,
            airlines_exclude: filters.airlines_exclude,
            connecting_airports: filters.connecting_airports,
            stopover_min: filters.stopover_min,
            stopover_max: filters.stopover_max,
            duration_max: filters.duration_max,
            lower_emissions: filters.lower_emissions,
            departing_times: filters.departing_times,
        });
        self
    }

    pub fn travellers(mut self, travellers: Travelers) -> Self {
        self.travellers = travellers;
        self
    }

    pub fn travel_class(mut self, travel_class: TravelClass) -> Self {
        self.travel_class = travel_class;
        self
    }

    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = sort_order;
        self
    }

    pub fn currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Maximum total ticket price cap. `None` = no limit.
    pub fn max_price(mut self, max: i32) -> Self {
        self.max_price = Some(max);
        self
    }

    /// Baggage allowance: `(carry_on_count, checked_count)`.
    pub fn baggage(mut self, carry_on: u8, checked: u8) -> Self {
        self.baggage = Some((carry_on, checked));
        self
    }

    /// BCP-47 language subtag. Default: `"en"`.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// ISO 3166-1 alpha-2 country code. Default: `"GB"`.
    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = country.into();
        self
    }

    pub fn build(self) -> Result<MultiCityConfig> {
        if self.legs.len() < 2 {
            return Err(anyhow!("multi-city requires at least 2 legs"));
        }
        for (i, leg) in self.legs.iter().enumerate() {
            if leg.from.is_empty() {
                return Err(anyhow!("leg {i} has no departure airport"));
            }
            if leg.to.is_empty() {
                return Err(anyhow!("leg {i} has no destination airport"));
            }
        }
        Ok(MultiCityConfig {
            legs: self.legs,
            travellers: self.travellers,
            travel_class: self.travel_class,
            sort_order: self.sort_order,
            currency: self.currency.unwrap_or_default(),
            max_price: self.max_price,
            baggage: self.baggage,
            language: if self.language.is_empty() {
                "en".to_string()
            } else {
                self.language
            },
            country: if self.country.is_empty() {
                "GB".to_string()
            } else {
                self.country
            },
        })
    }
}

/// Leg `tail` classifier used in the per-leg wire array (position \[14\]).
///
/// Observed rule from captured requests:
/// - `1` for the first leg, and for any leg whose departure airport matches
///   the first leg's departure (i.e. the traveller is departing from "home" again).
/// - `3` for all other legs.
pub fn leg_tail(index: usize, leg: &MultiCityLeg, first_leg: &MultiCityLeg) -> i32 {
    if index == 0 {
        return 1;
    }
    let first_origins: Vec<&str> = first_leg
        .from
        .iter()
        .map(|l| l.loc_identifier.as_str())
        .collect();
    let any_match = leg
        .from
        .iter()
        .any(|l| first_origins.contains(&l.loc_identifier.as_str()));
    if any_match {
        1
    } else {
        3
    }
}

/// Convert `MultiCityConfig` legs to protobuf `Leg` structs for URL encoding.
impl From<&MultiCityConfig> for Vec<Leg> {
    fn from(config: &MultiCityConfig) -> Vec<Leg> {
        config
            .legs
            .iter()
            .map(|leg| Leg {
                date: leg.date.to_string(),
                departure: leg.from.iter().map(loc_to_proto).collect(),
                arrival: leg.to.iter().map(loc_to_proto).collect(),
                min_hour_departure: None,
                max_hour_departure: None,
                min_hour_arrival: None,
                max_hour_arrival: None,
                stops: None,
                max_stopover_minutes: None,
                max_duration_minutes: None,
            })
            .collect()
    }
}

fn loc_to_proto(loc: &Location) -> crate::protos::urls::Location {
    let loc_type = match loc.loc_type {
        PlaceType::Airport => 1,
        _ => 2,
    };
    crate::protos::urls::Location {
        r#type: loc_type,
        place_name: loc.loc_identifier.clone(),
    }
}

/// Build the `ItineraryUrl` protobuf for multi-city URL encoding.
impl From<&MultiCityConfig> for ItineraryUrl {
    fn from(config: &MultiCityConfig) -> Self {
        let legs: Vec<Leg> = config.into();
        ItineraryUrl {
            legs,
            travellers: config.travellers.to_proto_vec(),
            class: config.travel_class as i32,
            trip_type: 3, // multi_city
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn airport(code: &str) -> Location {
        Location {
            loc_identifier: code.to_string(),
            loc_type: PlaceType::Airport,
            location_name: Some(code.to_string()),
        }
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn build_requires_two_legs() {
        let err = MultiCityConfig::builder()
            .add_leg_locations(
                vec![airport("LUX")],
                vec![airport("FCO")],
                date("2026-09-10"),
            )
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("2 legs"));
    }

    #[test]
    fn build_succeeds_with_two_legs() {
        let cfg = MultiCityConfig::builder()
            .add_leg_locations(
                vec![airport("LUX")],
                vec![airport("FCO")],
                date("2026-09-10"),
            )
            .add_leg_locations(
                vec![airport("FCO")],
                vec![airport("MAD")],
                date("2026-09-13"),
            )
            .build()
            .unwrap();
        assert_eq!(cfg.legs.len(), 2);
    }

    fn leg(from: &str, to: &str, d: &str) -> MultiCityLeg {
        let filters = LegFilters::default();
        MultiCityLeg {
            from: vec![airport(from)],
            to: vec![airport(to)],
            date: date(d),
            stop_options: filters.stop_options,
            airlines_include: filters.airlines_include,
            airlines_exclude: filters.airlines_exclude,
            connecting_airports: filters.connecting_airports,
            stopover_min: filters.stopover_min,
            stopover_max: filters.stopover_max,
            duration_max: filters.duration_max,
            lower_emissions: filters.lower_emissions,
            departing_times: filters.departing_times,
        }
    }

    #[test]
    fn leg_tail_first_is_one() {
        let first = leg("LUX", "FCO", "2026-09-10");
        assert_eq!(leg_tail(0, &first, &first), 1);
    }

    #[test]
    fn leg_tail_three_leg_pattern() {
        let leg0 = leg("LUX", "FCO", "2026-09-10");
        let leg1 = leg("FCO", "MAD", "2026-09-13");
        let leg2 = leg("MAD", "LUX", "2026-09-17");

        // 3-leg: LUX→FCO→MAD→LUX   tails should be [1, 3, 3]
        assert_eq!(leg_tail(0, &leg0, &leg0), 1);
        assert_eq!(leg_tail(1, &leg1, &leg0), 3);
        assert_eq!(leg_tail(2, &leg2, &leg0), 3);
    }

    #[test]
    fn leg_tail_four_leg_pattern() {
        let leg0 = leg("LUX", "FCO", "2026-09-10");
        let leg1 = leg("FCO", "MAD", "2026-09-13");
        let leg2 = leg("MAD", "LUX", "2026-09-17");
        let leg3 = leg("LUX", "STN", "2026-09-20");

        // 4-leg: LUX→FCO→MAD→LUX→STN   tails should be [1, 3, 3, 1]
        assert_eq!(leg_tail(0, &leg0, &leg0), 1);
        assert_eq!(leg_tail(1, &leg1, &leg0), 3);
        assert_eq!(leg_tail(2, &leg2, &leg0), 3);
        assert_eq!(leg_tail(3, &leg3, &leg0), 1); // LUX == first origin → 1
    }

    #[test]
    fn itinerary_url_trip_type_is_3() {
        let cfg = MultiCityConfig::builder()
            .add_leg_locations(
                vec![airport("LUX")],
                vec![airport("FCO")],
                date("2026-09-10"),
            )
            .add_leg_locations(
                vec![airport("FCO")],
                vec![airport("MAD")],
                date("2026-09-13"),
            )
            .build()
            .unwrap();
        let url = ItineraryUrl::from(&cfg);
        assert_eq!(url.trip_type, 3);
        assert_eq!(url.legs.len(), 2);
    }

    /// Verify that `LegFilters::default()` produces all-unrestricted values.
    #[test]
    fn leg_filters_default_is_unrestricted() {
        let f = LegFilters::default();
        assert!(matches!(f.stop_options, StopOptions::All));
        assert!(f.airlines_include.is_empty());
        assert!(f.airlines_exclude.is_empty());
        assert!(f.connecting_airports.is_empty());
        assert!(!f.lower_emissions);
    }

    /// Verify that `StopOptions::NoStop` on leg 0 produces a non-null value at
    /// wire position [3] in the serialised multi-city leg array.
    ///
    /// Wire format:
    ///   [dep, arr, times, stops(3), inc(4), exc(5), date(6), dur(7),
    ///    null, conn(9), null, min_stop(11), max_stop(12), emissions(13), tail(14)]
    #[test]
    fn multi_city_nonstop_leg_serializes_at_position_3() {
        use crate::parsers::common::ToRequestBody;
        use crate::parsers::request::flight_request::MultiCityRequestOptions;

        let filters = LegFilters {
            stop_options: StopOptions::NoStop,
            ..LegFilters::default()
        };
        let mut leg0 = leg("LUX", "FCO", "2026-09-10");
        leg0.stop_options = filters.stop_options;

        let leg1 = leg("FCO", "MAD", "2026-09-13");

        let cfg = MultiCityConfig {
            legs: vec![leg0, leg1],
            travellers: Travelers::new(vec![1, 0, 0, 0]).unwrap(),
            travel_class: TravelClass::Economy,
            sort_order: SortOrder::Best,
            currency: Currency::default(),
            max_price: None,
            baggage: None,
            language: "en".to_string(),
            country: "GB".to_string(),
        };

        let opts = MultiCityRequestOptions {
            config: &cfg,
            frontend_version: "test-version",
        };
        let body = opts.to_request_body().unwrap();

        // The serialised body is percent-encoded; decode it to inspect positions.
        let decoded = percent_encoding::percent_decode_str(&body.body)
            .decode_utf8_lossy()
            .to_string();

        // `StopOptions::NoStop` serialises to `1` (non-zero), while `StopOptions::All`
        // serialises to `0`.  Confirm that `1` appears in the first leg's array and
        // that `0` appears in the second leg's (which uses the default `All`).
        //
        // The decoded body contains the raw wire arrays — both legs are present.
        // We check that the non-stop value (1) appears somewhere before the second
        // leg (which starts at the second date marker "2026-09-13").
        let first_leg_section = decoded
            .split("2026-09-13")
            .next()
            .expect("date separator not found in body");
        assert!(
            first_leg_section.contains(",1,"),
            "NoStop (value 1) should appear in position [3] of the first leg wire array; \
             decoded first-leg section: {first_leg_section}"
        );
    }
}
