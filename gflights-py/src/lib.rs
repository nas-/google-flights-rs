//! Private Rust extension module — import via the `gflights` Python package.
//!
//! Build with `maturin develop` (editable install) or `maturin build --release`.
//!
//! ```python
//! import asyncio
//! import gflights
//!
//! async def main():
//!     client = gflights.GFlights()
//!     flights = await client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01")
//!     for f in flights:
//!         print(f.airline, f.duration_minutes, f.price)
//!
//! asyncio.run(main())
//! ```

// pyo3's generated wrapper functions emit a PyErr→PyErr conversion that triggers
// this lint. Suppress globally since we cannot annotate macro-generated items.
#![allow(clippy::useless_conversion)]

use anyhow::Context as _;
use chrono::{Months, NaiveDate};
use gflights::{
    parsers::common::{AirlineFilter, SortOrder, StopOptions, TravelClass},
    requests::{
        api::ApiClient,
        config::{Config, Currency},
    },
};
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_date(s: &str) -> PyResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} — expected YYYY-MM-DD"))
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

fn parse_currency(s: &str) -> PyResult<Currency> {
    <Currency as clap::ValueEnum>::from_str(s, true).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("unknown currency {s:?}: {e}"))
    })
}

fn parse_stop_options(s: &str) -> PyResult<StopOptions> {
    match s.to_lowercase().as_str() {
        "all" | "any" => Ok(StopOptions::All),
        "nonstop" | "non-stop" | "direct" => Ok(StopOptions::NoStop),
        "one-stop" | "onestop" | "one_stop" => Ok(StopOptions::OneOrLess),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown stop option {s:?} — use 'all', 'nonstop', or 'one-stop'"
        ))),
    }
}

fn parse_travel_class(s: &str) -> PyResult<TravelClass> {
    match s.to_lowercase().as_str() {
        "economy" | "eco" => Ok(TravelClass::Economy),
        "premium-economy" | "premium_economy" | "premiumeconomy" => Ok(TravelClass::PremiumEconomy),
        "business" | "biz" => Ok(TravelClass::Business),
        "first" | "first-class" => Ok(TravelClass::First),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown travel class {s:?} — use 'economy', 'premium-economy', 'business', or 'first'"
        ))),
    }
}

fn parse_sort_order(s: &str) -> PyResult<SortOrder> {
    match s.to_lowercase().as_str() {
        "best" => Ok(SortOrder::Best),
        "price" | "cheapest" => Ok(SortOrder::Price),
        "duration" | "shortest" => Ok(SortOrder::Duration),
        "departure" | "departure-time" | "dep" => Ok(SortOrder::DepartureTime),
        "arrival" | "arrival-time" | "arr" => Ok(SortOrder::ArrivalTime),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown sort order {s:?} — use 'best', 'price', 'duration', 'departure-time', or 'arrival-time'"
        ))),
    }
}

fn parse_airline_filters(list: &[String]) -> PyResult<Vec<AirlineFilter>> {
    list.iter()
        .map(|s| {
            s.parse::<AirlineFilter>().map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "invalid airline filter {s:?}: {e}"
                ))
            })
        })
        .collect()
}

fn anyhow_to_py(e: anyhow::Error) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------
// Python data classes
// ---------------------------------------------------------------------------

/// One flight leg (a single departure → arrival hop).
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct LegInfo {
    /// IATA code of the departure airport (e.g. `"LHR"`).
    pub from_airport: String,
    /// IATA code of the destination airport (e.g. `"JFK"`).
    pub to_airport: String,
    /// Departure time as `"HH:MM"` or empty string.
    pub departure_time: String,
    /// Arrival time as `"HH:MM"` or empty string.
    pub arrival_time: String,
    /// Departure date as `"YYYY-MM-DD"` or empty string.
    pub departure_date: String,
    /// Arrival date as `"YYYY-MM-DD"` or empty string.
    pub arrival_date: String,
    /// Duration of this individual leg in minutes, or `None` if not provided.
    pub duration_minutes: Option<i32>,
}

#[pymethods]
impl LegInfo {
    fn __repr__(&self) -> String {
        format!(
            "LegInfo(from={:?}, to={:?}, dep={} {}, arr={} {}, duration={:?})",
            self.from_airport,
            self.to_airport,
            self.departure_date,
            self.departure_time,
            self.arrival_date,
            self.arrival_time,
            self.duration_minutes,
        )
    }
}

/// Details about one layover / connection between two legs.
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct LayoverInfo {
    /// Minutes spent at the connecting airport.
    pub connection_minutes: i32,
    /// IATA code of the inbound (arrival) airport.
    pub arrival_airport: String,
    /// IATA code of the outbound (departure) airport (usually identical).
    pub departure_airport: String,
    /// `True` if this is an overnight layover.
    pub overnight: bool,
}

#[pymethods]
impl LayoverInfo {
    fn __repr__(&self) -> String {
        format!(
            "LayoverInfo(airport={:?}, {} min, overnight={})",
            self.arrival_airport, self.connection_minutes, self.overnight
        )
    }
}

/// CO₂ / emissions data for an itinerary (all values in grams).
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct EmissionsInfo {
    /// How much more (+) or less (−) CO₂ vs. typical route, as a percentage.
    pub vs_average_percent: Option<i64>,
    /// Estimated CO₂ for this specific flight, in grams.
    pub co2_this_flight_g: Option<i64>,
    /// Typical CO₂ for this route, in grams.
    pub co2_typical_route_g: Option<i64>,
    /// Lowest CO₂ found for this route, in grams.
    pub co2_lowest_route_g: Option<i64>,
}

#[pymethods]
impl EmissionsInfo {
    fn __repr__(&self) -> String {
        format!(
            "EmissionsInfo(vs_avg={:?}%, this={:?}g, typical={:?}g)",
            self.vs_average_percent, self.co2_this_flight_g, self.co2_typical_route_g,
        )
    }
}

/// One flight itinerary returned by :meth:`GFlights.search`.
#[pyclass]
#[derive(Clone, Debug)]
pub struct FlightResult {
    /// Primary operating carrier code (e.g. `"BA"`) or `"multi"` for codeshares.
    #[pyo3(get)]
    pub airline: String,
    /// Total door-to-door duration including all layovers, in minutes.
    #[pyo3(get)]
    pub duration_minutes: i64,
    /// Number of stops (0 = non-stop, 1 = one stop, etc.).
    #[pyo3(get)]
    pub stops: usize,
    /// Price in the requested currency, or `None` if unavailable.
    #[pyo3(get)]
    pub price: Option<i32>,
    /// Raw booking token — pass to :meth:`GFlights.offers` for booking URLs.
    #[pyo3(get)]
    pub booking_token: String,
    legs: Vec<LegInfo>,
    layovers: Vec<LayoverInfo>,
    emissions: Option<EmissionsInfo>,
}

#[pymethods]
impl FlightResult {
    /// List of individual flight legs.
    #[getter]
    fn legs(&self, py: Python<'_>) -> PyResult<Vec<Py<LegInfo>>> {
        self.legs.iter().map(|l| Py::new(py, l.clone())).collect()
    }

    /// List of layover connections between legs (empty for non-stop flights).
    #[getter]
    fn layovers(&self, py: Python<'_>) -> PyResult<Vec<Py<LayoverInfo>>> {
        self.layovers
            .iter()
            .map(|l| Py::new(py, l.clone()))
            .collect()
    }

    /// CO₂ emissions data, or `None` if Google did not return it.
    #[getter]
    fn emissions(&self, py: Python<'_>) -> PyResult<Option<Py<EmissionsInfo>>> {
        self.emissions
            .as_ref()
            .map(|e| Py::new(py, e.clone()))
            .transpose()
    }

    fn __repr__(&self) -> String {
        format!(
            "FlightResult(airline={:?}, duration={}min, stops={}, price={:?})",
            self.airline, self.duration_minutes, self.stops, self.price,
        )
    }
}

/// One (date, price) entry from the price graph (cheapest fare per day).
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct PriceEntry {
    /// Departure date as `"YYYY-MM-DD"`.
    pub date: String,
    /// Cheapest fare on that date, in the requested currency.
    pub price: i32,
}

#[pymethods]
impl PriceEntry {
    fn __repr__(&self) -> String {
        format!("PriceEntry(date={:?}, price={})", self.date, self.price)
    }
}

/// One cell in the departure × return date grid.
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct DateGridEntry {
    /// Outbound departure date as `"YYYY-MM-DD"`.
    pub dep_date: String,
    /// Return departure date as `"YYYY-MM-DD"`.
    pub ret_date: String,
    /// Cheapest fare for this (dep, ret) combination, in the requested currency.
    pub price: i32,
}

#[pymethods]
impl DateGridEntry {
    fn __repr__(&self) -> String {
        format!(
            "DateGridEntry(dep={:?}, ret={:?}, price={})",
            self.dep_date, self.ret_date, self.price,
        )
    }
}

// ---------------------------------------------------------------------------
// Conversions from gflights types to Python types
// ---------------------------------------------------------------------------

fn flight_info_to_leg(fi: &gflights::parsers::response::flight_response::FlightInfo) -> LegInfo {
    let dep_h = fi.departure_time.hour.unwrap_or(0);
    let arr_h = fi.arrival_time.hour.unwrap_or(0);
    LegInfo {
        from_airport: fi.departure_airport_code.clone(),
        to_airport: fi.destination_airport_code.clone(),
        departure_time: format!("{dep_h:02}:{:02}", fi.departure_time.minute),
        arrival_time: format!("{arr_h:02}:{:02}", fi.arrival_time.minute),
        departure_date: format!(
            "{:04}-{:02}-{:02}",
            fi.departure_date.year, fi.departure_date.month, fi.departure_date.day
        ),
        arrival_date: format!(
            "{:04}-{:02}-{:02}",
            fi.arrival_date.year, fi.arrival_date.month, fi.arrival_date.day
        ),
        duration_minutes: fi.leg_duration_minutes,
    }
}

fn conn_to_layover(
    c: &gflights::parsers::response::flight_response::ConnectionInfo,
) -> LayoverInfo {
    LayoverInfo {
        connection_minutes: c.connection_time_minutes,
        arrival_airport: c.arrival_airport.clone(),
        departure_airport: c.departure_airport.clone(),
        overnight: c
            .connection_warnings
            .as_ref()
            .is_some_and(|w| w.contains(&1)),
    }
}

fn emissions_to_py(e: &gflights::parsers::response::flight_response::Emissions) -> EmissionsInfo {
    EmissionsInfo {
        vs_average_percent: e.emission_vs_average_percent,
        co2_this_flight_g: e.co2_this_flight_g,
        co2_typical_route_g: e.co2_typical_route_g,
        co2_lowest_route_g: e.co2_lowest_route_g,
    }
}

fn itinerary_container_to_flight(
    ic: &gflights::parsers::response::flight_response::ItineraryContainer,
) -> FlightResult {
    FlightResult {
        airline: ic.itinerary.flight_by.clone(),
        duration_minutes: ic.itinerary.total_time_minutes,
        stops: ic.itinerary.stop_count(),
        price: ic.itinerary_cost.trip_cost.as_ref().map(|c| c.price),
        booking_token: ic.itinerary_cost.departure_token.clone(),
        legs: ic
            .itinerary
            .flight_details
            .iter()
            .map(flight_info_to_leg)
            .collect(),
        layovers: ic
            .itinerary
            .connection_info
            .as_ref()
            .map(|v| v.iter().map(conn_to_layover).collect())
            .unwrap_or_default(),
        emissions: ic.itinerary.emissions.as_ref().map(emissions_to_py),
    }
}

// ---------------------------------------------------------------------------
// Main Python class
// ---------------------------------------------------------------------------

/// Async Python client for Google Flights, backed by a fast Rust/tokio core.
///
/// All three search methods are coroutines — use with ``await``.
/// Multiple calls can run concurrently with ``asyncio.gather``.
///
/// Example::
///
///     import asyncio, gflights
///
///     async def main():
///         client = gflights.GFlights()
///         lhr_jfk, mad_mex = await asyncio.gather(
///             client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01"),
///             client.search(from_airport="MAD", to_airport="MEX", date="2026-08-01"),
///         )
///
///     asyncio.run(main())
#[pyclass]
pub struct GFlights {
    client: ApiClient,
}

#[pymethods]
impl GFlights {
    /// Create a new client.
    ///
    /// The constructor is synchronous (fast — just initialises the HTTP client).
    /// All search methods are async coroutines.
    #[new]
    fn new() -> Self {
        let client = pyo3_async_runtimes::tokio::get_runtime().block_on(ApiClient::new());
        GFlights { client }
    }

    /// Search for flights.
    ///
    /// :param from_airport: Departure IATA code or city name (e.g. ``"LHR"``, ``"London"``).
    /// :param to_airport:   Destination IATA code or city name.
    /// :param date:         Departure date as ``"YYYY-MM-DD"``.
    /// :param return_date:  Return date for round-trips.  ``None`` for one-way.
    /// :param adults:       Number of adult passengers (default 1).
    /// :param travel_class: ``"economy"`` / ``"premium-economy"`` / ``"business"`` / ``"first"``.
    /// :param stops:        ``"all"`` / ``"nonstop"`` / ``"one-stop"``.
    /// :param sort:         ``"best"`` / ``"price"`` / ``"duration"`` / ``"departure-time"`` / ``"arrival-time"``.
    /// :param airlines_include: IATA codes or alliances to include (e.g. ``["BA", "ONEWORLD"]``).
    /// :param airlines_exclude: IATA codes or alliances to exclude.
    /// :param via:          Require a connection through these airports.
    /// :param lower_emissions: Restrict to below-average CO₂ flights.
    /// :param currency:     Currency name (e.g. ``"euro"``, ``"us-dollar"``).
    /// :param lang:         BCP-47 language subtag (default ``"en"``).
    /// :param country:      ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:            Coroutine → ``list[FlightResult]``
    #[pyo3(signature = (
        from_airport,
        to_airport,
        date,
        return_date = None,
        adults = 1,
        travel_class = "economy",
        stops = "all",
        sort = "best",
        airlines_include = vec![],
        airlines_exclude = vec![],
        via = vec![],
        lower_emissions = false,
        currency = "euro",
        lang = "en",
        country = "GB",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn search<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        to_airport: String,
        date: String,
        return_date: Option<String>,
        adults: u8,
        travel_class: &str,
        stops: &str,
        sort: &str,
        airlines_include: Vec<String>,
        airlines_exclude: Vec<String>,
        via: Vec<String>,
        lower_emissions: bool,
        currency: &str,
        lang: &str,
        country: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Validate synchronously — raises ValueError before the coroutine is even awaited.
        let dep_date = parse_date(&date)?;
        let ret_date = return_date.as_deref().map(parse_date).transpose()?;
        let currency = parse_currency(currency)?;
        let stop_opt = parse_stop_options(stops)?;
        let class = parse_travel_class(travel_class)?;
        let sort_ord = parse_sort_order(sort)?;
        let inc = parse_airline_filters(&airlines_include)?;
        let exc = parse_airline_filters(&airlines_exclude)?;
        let travelers = gflights::parsers::common::Travelers::new(vec![adults.into(), 0, 0, 0])
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        // Convert &str → String before moving into the 'static async block.
        let lang = lang.to_string();
        let country = country.to_string();
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut builder = Config::builder()
                .departure(&from_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .destination(&to_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .departing_date(dep_date)
                .travel_class(class)
                .stop_options(stop_opt)
                .sort_order(sort_ord)
                .currency(currency)
                .language(&lang)
                .country(&country)
                .travelers(travelers)
                .airlines_include(inc)
                .airlines_exclude(exc)
                .lower_emissions(lower_emissions);

            for airport in &via {
                builder = builder.add_connecting_airport(airport);
            }
            if let Some(r) = ret_date {
                builder = builder.return_date(r);
            }

            let config = builder.build().map_err(anyhow_to_py)?;
            let flights = client
                .request_flights(&config)
                .await
                .map_err(anyhow_to_py)?
                .get_all_flights();

            Python::with_gil(|py| {
                flights
                    .iter()
                    .map(|ic| Py::new(py, itinerary_container_to_flight(ic)))
                    .collect::<PyResult<Vec<_>>>()
            })
        })
    }

    /// Retrieve the cheapest fare for each day over a date range (price graph).
    ///
    /// :param from_airport: Departure IATA code or city name.
    /// :param to_airport:   Destination IATA code or city name.
    /// :param date:         Start date as ``"YYYY-MM-DD"``.
    /// :param months:       How many months of price data to fetch (default 1).
    /// :param currency:     Currency name (e.g. ``"euro"``).
    /// :param lang:         BCP-47 language subtag.
    /// :param country:      ISO 3166-1 alpha-2 country code.
    /// :returns:            Coroutine → ``list[PriceEntry]``, sorted by date.
    #[pyo3(signature = (
        from_airport,
        to_airport,
        date,
        months = 1,
        currency = "euro",
        lang = "en",
        country = "GB",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn price_graph<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        to_airport: String,
        date: String,
        months: u32,
        currency: &str,
        lang: &str,
        country: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dep_date = parse_date(&date)?;
        let currency = parse_currency(currency)?;
        let lang = lang.to_string();
        let country = country.to_string();
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let config = Config::builder()
                .departure(&from_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .destination(&to_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .departing_date(dep_date)
                .currency(currency)
                .language(&lang)
                .country(&country)
                .build()
                .map_err(anyhow_to_py)?;

            let graph = client
                .request_graph(&config, Months::new(months))
                .await
                .map_err(anyhow_to_py)?;

            let mut entries: Vec<PriceEntry> = graph
                .get_all_graphs()
                .iter()
                .filter_map(|g| {
                    let (date, price) = g.maybe_get_date_price()?;
                    Some(PriceEntry {
                        date: date.to_string(),
                        price,
                    })
                })
                .collect();
            entries.sort_by(|a, b| a.date.cmp(&b.date));

            Python::with_gil(|py| {
                entries
                    .into_iter()
                    .map(|e| Py::new(py, e))
                    .collect::<PyResult<Vec<_>>>()
            })
        })
    }

    /// Retrieve the cheapest fare for every (departure × return date) combination.
    ///
    /// :param from_airport: Departure IATA code or city name.
    /// :param to_airport:   Destination IATA code or city name.
    /// :param dep_start:    First outbound departure date ``"YYYY-MM-DD"``.
    /// :param dep_end:      Last outbound departure date ``"YYYY-MM-DD"``.
    /// :param ret_start:    First return date ``"YYYY-MM-DD"``.
    /// :param ret_end:      Last return date ``"YYYY-MM-DD"``.
    /// :param currency:     Currency name (e.g. ``"euro"``).
    /// :param lang:         BCP-47 language subtag.
    /// :param country:      ISO 3166-1 alpha-2 country code.
    /// :returns:            Coroutine → ``list[DateGridEntry]``
    #[pyo3(signature = (
        from_airport,
        to_airport,
        dep_start,
        dep_end,
        ret_start,
        ret_end,
        currency = "euro",
        lang = "en",
        country = "GB",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn date_grid<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        to_airport: String,
        dep_start: String,
        dep_end: String,
        ret_start: String,
        ret_end: String,
        currency: &str,
        lang: &str,
        country: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dep_s = parse_date(&dep_start)?;
        let dep_e = parse_date(&dep_end)?;
        let ret_s = parse_date(&ret_start)?;
        let ret_e = parse_date(&ret_end)?;
        let currency = parse_currency(currency)?;
        let lang = lang.to_string();
        let country = country.to_string();
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let config = Config::builder()
                .departure(&from_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .destination(&to_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .departing_date(dep_s)
                .return_date(ret_s)
                .currency(currency)
                .language(&lang)
                .country(&country)
                .build()
                .map_err(anyhow_to_py)?;

            let grid = client
                .request_date_grid(&config, dep_s, dep_e, ret_s, ret_e)
                .await
                .map_err(anyhow_to_py)?;

            Python::with_gil(|py| {
                grid.entries
                    .into_iter()
                    .map(|e| {
                        Py::new(
                            py,
                            DateGridEntry {
                                dep_date: e.departure_date.to_string(),
                                ret_date: e.return_date.to_string(),
                                price: e.price,
                            },
                        )
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
        })
    }

    /// `True` if the last request was rate-limited by Google (HTTP 429).
    #[getter]
    fn rate_limited(&self) -> bool {
        self.client.is_rate_limited()
    }

    /// Reset the rate-limit flag after a cooling-off period.
    fn reset_rate_limit(&self) {
        self.client.reset_rate_limit();
    }

    fn __repr__(&self) -> &'static str {
        "GFlights()"
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Private Rust extension — import via the `gflights` Python package.
#[pymodule]
fn _gflights(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialise the shared multi-thread tokio runtime used by all coroutines.
    // init() takes a Builder (not a built Runtime) and builds it internally.
    let mut rt_builder = tokio::runtime::Builder::new_multi_thread();
    rt_builder.enable_all();
    pyo3_async_runtimes::tokio::init(rt_builder);

    m.add_class::<GFlights>()?;
    m.add_class::<FlightResult>()?;
    m.add_class::<LegInfo>()?;
    m.add_class::<LayoverInfo>()?;
    m.add_class::<EmissionsInfo>()?;
    m.add_class::<PriceEntry>()?;
    m.add_class::<DateGridEntry>()?;
    Ok(())
}
