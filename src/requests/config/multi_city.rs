use anyhow::{anyhow, Result};
use chrono::NaiveDate;

use crate::parsers::common::{Location, PlaceType, SortOrder, TravelClass, Travelers};
use crate::protos::urls::{ItineraryUrl, Leg};
use crate::requests::api::ApiClient;

use super::builder::get_location_pub;
use super::currency::Currency;

/// A single leg in a multi-city itinerary.
#[derive(Debug, Clone)]
pub struct MultiCityLeg {
    /// One to four origin airports / city identifiers.
    pub from: Vec<Location>,
    /// One to four destination airports / city identifiers.
    pub to: Vec<Location>,
    pub date: NaiveDate,
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
        self.legs.push(MultiCityLeg {
            from: vec![from_loc],
            to: vec![to_loc],
            date,
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
        self.legs.push(MultiCityLeg { from, to, date });
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

/// Leg `tail` classifier used in the per-leg wire array (position [14]).
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

    #[test]
    fn leg_tail_first_is_one() {
        let first = MultiCityLeg {
            from: vec![airport("LUX")],
            to: vec![airport("FCO")],
            date: date("2026-09-10"),
        };
        assert_eq!(leg_tail(0, &first, &first), 1);
    }

    #[test]
    fn leg_tail_three_leg_pattern() {
        let lux = airport("LUX");
        let fco = airport("FCO");
        let mad = airport("MAD");
        let leg0 = MultiCityLeg {
            from: vec![lux.clone()],
            to: vec![fco.clone()],
            date: date("2026-09-10"),
        };
        let leg1 = MultiCityLeg {
            from: vec![fco.clone()],
            to: vec![mad.clone()],
            date: date("2026-09-13"),
        };
        let leg2 = MultiCityLeg {
            from: vec![mad.clone()],
            to: vec![lux.clone()],
            date: date("2026-09-17"),
        };

        // 3-leg: LUX→FCO→MAD→LUX   tails should be [1, 3, 3]
        assert_eq!(leg_tail(0, &leg0, &leg0), 1);
        assert_eq!(leg_tail(1, &leg1, &leg0), 3);
        assert_eq!(leg_tail(2, &leg2, &leg0), 3);
    }

    #[test]
    fn leg_tail_four_leg_pattern() {
        let lux = airport("LUX");
        let fco = airport("FCO");
        let mad = airport("MAD");
        let stn = airport("STN");
        let leg0 = MultiCityLeg {
            from: vec![lux.clone()],
            to: vec![fco.clone()],
            date: date("2026-09-10"),
        };
        let leg1 = MultiCityLeg {
            from: vec![fco.clone()],
            to: vec![mad.clone()],
            date: date("2026-09-13"),
        };
        let leg2 = MultiCityLeg {
            from: vec![mad.clone()],
            to: vec![lux.clone()],
            date: date("2026-09-17"),
        };
        let leg3 = MultiCityLeg {
            from: vec![lux.clone()],
            to: vec![stn.clone()],
            date: date("2026-09-20"),
        };

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
}
