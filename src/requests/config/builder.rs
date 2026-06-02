use anyhow::{anyhow, Result};
use chrono::NaiveDate;

use crate::parsers::common::{
    AirlineFilter, FixedFlights, FlightTimes, Location, PlaceType, SortOrder, StopOptions,
    StopoverDuration, TotalDuration, TravelClass, Travelers,
};
use crate::requests::api::ApiClient;

use super::{Config, Currency, TripType};

/// Builder for [`Config`].  Obtain one via [`Config::builder()`].
pub struct ConfigBuilder {
    pub(super) departing_date: Option<NaiveDate>,
    pub(super) departure: Vec<Location>,
    pub(super) destination: Vec<Location>,
    pub(super) stop_options: StopOptions,
    pub(super) travel_class: TravelClass,
    pub(super) return_date: Option<NaiveDate>,
    pub(super) travelers: Travelers,
    pub(super) departing_times: FlightTimes,
    pub(super) return_times: FlightTimes,
    pub(super) stopover_max: StopoverDuration,
    /// Minimum layover duration. Default: no minimum.
    pub(super) stopover_min: StopoverDuration,
    pub(super) duration_max: TotalDuration,
    pub(super) currency: Option<Currency>,
    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`. Default: `"en"`.
    pub(super) language: String,
    /// ISO 3166-1 alpha-2 country code, e.g. `"GB"`, `"FR"`. Default: `"GB"`.
    pub(super) country: String,
    /// Sort order for search results. Default: [`SortOrder::Best`].
    pub(super) sort_order: SortOrder,
    /// Airlines / alliances to include. Empty = no restriction.
    pub(super) airlines_include: Vec<AirlineFilter>,
    /// Airlines / alliances to exclude. Empty = no restriction.
    pub(super) airlines_exclude: Vec<AirlineFilter>,
    /// Connecting airport IATA codes. Empty = no restriction.
    pub(super) connecting_airports: Vec<String>,
    /// Restrict to lower-CO₂ emissions flights. Default: `false`.
    pub(super) lower_emissions: bool,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            departing_date: None,
            departure: Vec::new(),
            destination: Vec::new(),
            stop_options: StopOptions::default(),
            travel_class: TravelClass::default(),
            return_date: None,
            travelers: Travelers::default(),
            departing_times: FlightTimes::default(),
            return_times: FlightTimes::default(),
            stopover_max: StopoverDuration::default(),
            stopover_min: StopoverDuration::default(),
            duration_max: TotalDuration::default(),
            currency: None,
            language: "en".to_string(),
            country: "GB".to_string(),
            sort_order: SortOrder::default(),
            airlines_include: Vec::new(),
            airlines_exclude: Vec::new(),
            connecting_airports: Vec::new(),
            lower_emissions: false,
        }
    }
}

impl ConfigBuilder {
    pub fn departing_date(mut self, date: NaiveDate) -> Self {
        self.departing_date = Some(date);
        self
    }

    /// Set the departure to a single airport/city from a string, clearing any previously
    /// added departure airports.  Resolves IATA codes and city names via the network.
    pub async fn departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.departure = vec![loc];
        Ok(self)
    }

    /// Set the departure to a single [`Location`] directly, without a network lookup.
    ///
    /// Use this when you already have a `Location` (e.g. from a previous city lookup or
    /// when building tests without an [`ApiClient`]).
    pub fn departure_location(mut self, location: Location) -> Self {
        self.departure = vec![location];
        self
    }

    /// Add an additional departure airport/city (up to 4 total).
    /// Google Flights will search "any of these airports" as the origin.
    pub async fn add_departure(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        if self.departure.len() >= 4 {
            return Err(anyhow!("A maximum of 4 departure airports is supported"));
        }
        let loc = get_location(location, client).await?;
        self.departure.push(loc);
        Ok(self)
    }

    /// Set the destination to a single airport/city from a string, clearing any previously
    /// added destination airports.  Resolves IATA codes and city names via the network.
    pub async fn destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        let loc = get_location(location, client).await?;
        self.destination = vec![loc];
        Ok(self)
    }

    /// Set the destination to a single [`Location`] directly, without a network lookup.
    ///
    /// Use this when you already have a `Location` (e.g. from a previous city lookup or
    /// when building tests without an [`ApiClient`]).
    pub fn destination_location(mut self, location: Location) -> Self {
        self.destination = vec![location];
        self
    }

    /// Add an additional destination airport/city (up to 4 total).
    /// Google Flights will search "any of these airports" as the destination.
    pub async fn add_destination(mut self, location: &str, client: &ApiClient) -> Result<Self> {
        if self.destination.len() >= 4 {
            return Err(anyhow!("A maximum of 4 destination airports is supported"));
        }
        let loc = get_location(location, client).await?;
        self.destination.push(loc);
        Ok(self)
    }

    pub fn return_date(mut self, date: NaiveDate) -> Self {
        self.return_date = Some(date);
        self
    }

    pub fn travelers(mut self, travelers: Travelers) -> Self {
        self.travelers = travelers;
        self
    }

    pub fn stop_options(mut self, stop_options: StopOptions) -> Self {
        self.stop_options = stop_options;
        self
    }

    pub fn travel_class(mut self, travel_class: TravelClass) -> Self {
        self.travel_class = travel_class;
        self
    }

    pub fn departing_times(mut self, times: FlightTimes) -> Self {
        self.departing_times = times;
        self
    }

    pub fn return_times(mut self, times: FlightTimes) -> Self {
        self.return_times = times;
        self
    }

    pub fn stopover_max(mut self, duration: StopoverDuration) -> Self {
        self.stopover_max = duration;
        self
    }

    /// Set the minimum layover / connection duration.
    ///
    /// Use this to avoid very short layovers.  Google Flights lets you choose
    /// minimum layover times in 30-minute intervals.
    pub fn stopover_min(mut self, duration: StopoverDuration) -> Self {
        self.stopover_min = duration;
        self
    }

    pub fn duration_max(mut self, duration: TotalDuration) -> Self {
        self.duration_max = duration;
        self
    }

    pub fn currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Set the language for the Google Flights UI.
    ///
    /// Accepts a BCP-47 language subtag such as `"en"`, `"fr"`, `"de"`.
    /// Defaults to `"en"`.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Set the country for the Google Flights locale.
    ///
    /// Accepts an ISO 3166-1 alpha-2 code such as `"GB"`, `"FR"`, `"US"`.
    /// Case-insensitive — stored as-supplied, uppercased in requests.
    /// Defaults to `"GB"`.
    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = country.into();
        self
    }

    /// Set the sort order for flight search results.
    ///
    /// Defaults to [`SortOrder::Best`] (Google's composite ranking).
    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = sort_order;
        self
    }

    /// Replace the entire airlines-include list.
    ///
    /// Accepts IATA codes (`"LX"`, `"LH"`) and alliance names
    /// (`"ONEWORLD"`, `"SKYTEAM"`, `"STAR_ALLIANCE"`).
    pub fn airlines_include(mut self, filters: Vec<AirlineFilter>) -> Self {
        self.airlines_include = filters;
        self
    }

    /// Add a single airline/alliance to the include filter.
    pub fn add_airline_include(mut self, filter: AirlineFilter) -> Self {
        self.airlines_include.push(filter);
        self
    }

    /// Replace the entire airlines-exclude list.
    pub fn airlines_exclude(mut self, filters: Vec<AirlineFilter>) -> Self {
        self.airlines_exclude = filters;
        self
    }

    /// Add a single airline/alliance to the exclude filter.
    pub fn add_airline_exclude(mut self, filter: AirlineFilter) -> Self {
        self.airlines_exclude.push(filter);
        self
    }

    /// Set the list of connecting airports (IATA codes, e.g. `"CDG"`).
    ///
    /// Only itineraries that connect through at least one of these airports
    /// will be returned.
    pub fn connecting_airports(mut self, airports: Vec<String>) -> Self {
        self.connecting_airports = airports;
        self
    }

    /// Add a single connecting airport (IATA code).
    pub fn add_connecting_airport(mut self, airport: impl Into<String>) -> Self {
        self.connecting_airports.push(airport.into());
        self
    }

    /// If `true`, restrict results to flights with lower CO₂ emissions.
    ///
    /// Defaults to `false` (no restriction).
    pub fn lower_emissions(mut self, lower: bool) -> Self {
        self.lower_emissions = lower;
        self
    }

    pub fn build(self) -> Result<Config> {
        let departing_date = self
            .departing_date
            .ok_or(anyhow!("Departing date is required"))?;
        let trip_type = match self.return_date {
            Some(_) => TripType::Return,
            None => TripType::OneWay,
        };
        if self.departure.is_empty() {
            return Err(anyhow!("At least one departure airport is required"));
        }
        if self.destination.is_empty() {
            return Err(anyhow!("At least one destination airport is required"));
        }
        Ok(Config {
            departing_date,
            departure: self.departure,
            destination: self.destination,
            stop_options: self.stop_options,
            travel_class: self.travel_class,
            return_date: self.return_date,
            travellers: self.travelers,
            departing_times: self.departing_times,
            return_times: self.return_times,
            stopover_max: self.stopover_max,
            stopover_min: self.stopover_min,
            duration_max: self.duration_max,
            currency: self.currency.unwrap_or_default(),
            trip_type,
            fixed_flights: match trip_type {
                TripType::Return => FixedFlights::new(2),
                TripType::OneWay => FixedFlights::new(1),
                TripType::MultiCity => {
                    return Err(anyhow!("Multi-city trips are not yet implemented"));
                }
            },
            language: self.language,
            country: self.country,
            sort_order: self.sort_order,
            airlines_include: self.airlines_include,
            airlines_exclude: self.airlines_exclude,
            connecting_airports: self.connecting_airports,
            lower_emissions: self.lower_emissions,
        })
    }
}

pub(super) async fn get_location_pub(
    location: &str,
    client: &ApiClient,
) -> Result<Location, anyhow::Error> {
    get_location(location, client).await
}

async fn get_location(location: &str, client: &ApiClient) -> Result<Location, anyhow::Error> {
    let departure = if location.len() == 3 && location.chars().all(char::is_uppercase) {
        if location.starts_with('X') {
            eprintln!(
                "Warning: '{}' looks like a rail station code. \
                 If you meant a city, use the full name (e.g. 'Rome').",
                location
            );
        }
        Location {
            loc_identifier: location.to_owned(),
            loc_type: PlaceType::Airport,
            location_name: Some(location.to_string()),
        }
    } else {
        client.request_city(location).await?.to_city_list()
    };
    Ok(departure)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn future_date(days: i64) -> NaiveDate {
        Utc::now().date_naive() + Duration::days(days)
    }

    /// `build()` returns an error when no departure airport was set.
    #[test]
    fn build_fails_without_departure() {
        let result = Config::builder().departing_date(future_date(30)).build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("departure"), "error should mention departure");
    }

    /// `build()` returns an error when departure is set but destination is not.
    #[test]
    fn build_fails_without_destination() {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let result = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("destination"),
            "error should mention destination, got: {msg}"
        );
    }

    /// `build()` returns an error when neither departure nor destination is set.
    #[test]
    fn build_fails_without_both_airports() {
        let result = Config::builder().departing_date(future_date(30)).build();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("departure") || msg.contains("destination"),
            "error should mention a missing airport, got: {msg}"
        );
    }

    /// ConfigBuilder locale setters work and Config::default() starts with en/GB.
    #[test]
    fn config_default_locale_is_en_gb() {
        let cfg = Config::default();
        assert_eq!(cfg.language, "en");
        assert_eq!(cfg.country, "GB");
    }

    /// ConfigBuilder::language() and country() setters propagate to the built Config.
    #[test]
    fn builder_locale_setters_propagate() {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .language("fr")
            .country("FR")
            .build()
            .expect("valid config");
        assert_eq!(cfg.language, "fr");
        assert_eq!(cfg.country, "FR");
    }

    /// ConfigBuilder::sort_order() propagates the chosen sort order.
    #[test]
    fn builder_sort_order_setter_propagates() {
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .sort_order(SortOrder::Price)
            .build()
            .expect("valid config");
        assert!(matches!(cfg.sort_order, SortOrder::Price));
    }

    /// All ConfigBuilder filter setters propagate to the built Config.
    #[test]
    fn builder_filter_setters_propagate() {
        use crate::parsers::common::{AirlineCode, AirlineFilter, Alliance};
        let lhr = Location {
            loc_identifier: "LHR".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let jfk = Location {
            loc_identifier: "JFK".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: None,
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(lhr)
            .destination_location(jfk)
            .add_airline_include(AirlineFilter::Airline(AirlineCode::new("LX").unwrap()))
            .add_airline_include(AirlineFilter::Alliance(Alliance::OneWorld))
            .add_airline_exclude(AirlineFilter::Airline(AirlineCode::new("FR").unwrap()))
            .add_connecting_airport("CDG")
            .lower_emissions(true)
            .stopover_min(StopoverDuration::Minutes(60))
            .stopover_max(StopoverDuration::Minutes(180))
            .build()
            .expect("valid config");

        assert_eq!(cfg.airlines_include.len(), 2);
        assert_eq!(cfg.airlines_exclude.len(), 1);
        assert_eq!(cfg.connecting_airports, vec!["CDG"]);
        assert!(cfg.lower_emissions);
        assert!(matches!(cfg.stopover_min, StopoverDuration::Minutes(60)));
        assert!(matches!(cfg.stopover_max, StopoverDuration::Minutes(180)));
    }

    /// `departure_location` accepts an X-prefixed code (e.g. a rail station code)
    /// without error — the warning is printed to stderr but the Location is created
    /// and the Config builds successfully.  The warning path is exercised via the
    /// async `departure()` helper in live tests; here we verify the code path that
    /// already has a `Location` does not reject X-prefix codes.
    #[test]
    fn x_prefixed_location_builds_without_error() {
        let xrj = Location {
            loc_identifier: "XRJ".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: Some("XRJ".to_owned()),
        };
        let xvq = Location {
            loc_identifier: "XVQ".to_owned(),
            loc_type: PlaceType::Airport,
            location_name: Some("XVQ".to_owned()),
        };
        let cfg = Config::builder()
            .departing_date(future_date(30))
            .departure_location(xrj)
            .destination_location(xvq)
            .build()
            .expect("X-prefixed location codes must not be rejected by Config::build");
        assert_eq!(cfg.departure[0].loc_identifier, "XRJ");
        assert_eq!(cfg.destination[0].loc_identifier, "XVQ");
    }
}
