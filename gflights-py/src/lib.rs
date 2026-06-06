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
        config::{
            Config, Currency, DealConfig, ExploreConfig, ExploreDate, ExploreDuration,
            MultiCityConfig,
        },
    },
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_date(s: &str) -> PyResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} — expected YYYY-MM-DD"))
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

fn parse_currency(s: &str) -> PyResult<Currency> {
    Currency::from_code(s).ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "unknown currency {s:?} (expected an ISO-4217 code, e.g. \"USD\", \"EUR\", \"GBP\")"
        ))
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

// ---------------------------------------------------------------------------
// GFlightsError — typed exception raised by all API methods
// ---------------------------------------------------------------------------

pyo3::create_exception!(_gflights, GFlightsError, pyo3::exceptions::PyException);

fn anyhow_to_py(e: anyhow::Error) -> PyErr {
    GFlightsError::new_err(e.to_string())
}

/// Render an `Option<T>` for a Python-style `__repr__`: the inner value, or
/// the literal `None` — never Rust's `Some(..)` wrapper.
fn repr_opt<T: std::fmt::Display>(o: &Option<T>) -> String {
    match o {
        Some(v) => v.to_string(),
        None => "None".to_string(),
    }
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
            "LegInfo(from={:?}, to={:?}, dep={} {}, arr={} {}, duration={})",
            self.from_airport,
            self.to_airport,
            self.departure_date,
            self.departure_time,
            self.arrival_date,
            self.arrival_time,
            repr_opt(&self.duration_minutes),
        )
    }

    /// Return this leg as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("from_airport", &self.from_airport)?;
        d.set_item("to_airport", &self.to_airport)?;
        d.set_item("departure_time", &self.departure_time)?;
        d.set_item("arrival_time", &self.arrival_time)?;
        d.set_item("departure_date", &self.departure_date)?;
        d.set_item("arrival_date", &self.arrival_date)?;
        d.set_item("duration_minutes", self.duration_minutes)?;
        Ok(d)
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
            self.arrival_airport,
            self.connection_minutes,
            if self.overnight { "True" } else { "False" },
        )
    }

    /// Return this layover as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("connection_minutes", self.connection_minutes)?;
        d.set_item("arrival_airport", &self.arrival_airport)?;
        d.set_item("departure_airport", &self.departure_airport)?;
        d.set_item("overnight", self.overnight)?;
        Ok(d)
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
            "EmissionsInfo(vs_average_percent={}, co2_this_flight_g={}, co2_typical_route_g={}, co2_lowest_route_g={})",
            repr_opt(&self.vs_average_percent),
            repr_opt(&self.co2_this_flight_g),
            repr_opt(&self.co2_typical_route_g),
            repr_opt(&self.co2_lowest_route_g),
        )
    }

    /// Return these emissions figures as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("vs_average_percent", self.vs_average_percent)?;
        d.set_item("co2_this_flight_g", self.co2_this_flight_g)?;
        d.set_item("co2_typical_route_g", self.co2_typical_route_g)?;
        d.set_item("co2_lowest_route_g", self.co2_lowest_route_g)?;
        Ok(d)
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
            "FlightResult(airline={:?}, duration={}min, stops={}, price={})",
            self.airline,
            self.duration_minutes,
            self.stops,
            repr_opt(&self.price),
        )
    }

    /// Return the full itinerary as a nested plain ``dict`` (legs, layovers and
    /// emissions are recursively expanded into dicts/lists).
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("airline", &self.airline)?;
        d.set_item("duration_minutes", self.duration_minutes)?;
        d.set_item("stops", self.stops)?;
        d.set_item("price", self.price)?;
        d.set_item("booking_token", &self.booking_token)?;
        let legs = PyList::empty(py);
        for l in &self.legs {
            legs.append(l.to_dict(py)?)?;
        }
        d.set_item("legs", legs)?;
        let layovers = PyList::empty(py);
        for l in &self.layovers {
            layovers.append(l.to_dict(py)?)?;
        }
        d.set_item("layovers", layovers)?;
        let emissions = match &self.emissions {
            Some(e) => Some(e.to_dict(py)?),
            None => None,
        };
        d.set_item("emissions", emissions)?;
        Ok(d)
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

    /// Return this entry as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("date", &self.date)?;
        d.set_item("price", self.price)?;
        Ok(d)
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

    /// Return this grid cell as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("dep_date", &self.dep_date)?;
        d.set_item("ret_date", &self.ret_date)?;
        d.set_item("price", self.price)?;
        Ok(d)
    }
}

/// One result from `GFlights.cheapest_dates`.
///
/// `return_date` is `None` for one-way searches and set for round-trip searches.
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct CheapDate {
    /// Cheapest outbound departure date as `"YYYY-MM-DD"`.
    pub departure_date: String,
    /// Return date as `"YYYY-MM-DD"`, or `None` for one-way results.
    pub return_date: Option<String>,
    /// Cheapest fare in the requested currency.
    pub price: i32,
}

#[pymethods]
impl CheapDate {
    fn __repr__(&self) -> String {
        match &self.return_date {
            Some(ret) => format!(
                "CheapDate(dep={:?}, ret={:?}, price={})",
                self.departure_date, ret, self.price,
            ),
            None => format!(
                "CheapDate(dep={:?}, price={})",
                self.departure_date, self.price,
            ),
        }
    }

    /// Return this cheapest-date result as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("departure_date", &self.departure_date)?;
        d.set_item("return_date", &self.return_date)?;
        d.set_item("price", self.price)?;
        Ok(d)
    }
}

/// One explore destination returned by :meth:`GFlights.explore`.
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct ExploreResult {
    /// Google Knowledge-Graph place ID (e.g. ``"/m/0vzm"`` for Vienna).
    pub place_id: String,
    /// English destination name.
    pub name: String,
    /// Country name.
    pub country: String,
    /// Latitude of the destination.
    pub lat: f64,
    /// Longitude of the destination.
    pub lng: f64,
    /// URL of a cover photo, or ``None``.
    pub image_url: Option<String>,
    /// IATA code of the nearest airport to the destination (geographic label).
    pub nearest_airport: String,
    /// IATA code of the airport the priced flight actually lands at, or ``None``.
    ///
    /// Prefer this over ``nearest_airport`` when booking or displaying to users —
    /// they can differ when Google prices a flight to a secondary airport
    /// (e.g. ``nearest_airport="NCE"`` but ``flight_airport="MRS"``).
    pub flight_airport: Option<String>,
    /// Earliest outbound departure date as ``"YYYY-MM-DD"``, or ``None``.
    pub date_from: Option<String>,
    /// Latest return date as ``"YYYY-MM-DD"``, or ``None``.
    pub date_to: Option<String>,
    /// Cheapest round-trip price, or ``None``.
    pub price: Option<i32>,
    /// Primary operating airline code, or ``None``.
    pub airline: Option<String>,
    /// Number of stops on the outbound leg, or ``None``.
    pub stops: Option<u8>,
    /// Outbound flight duration in minutes, or ``None``.
    pub flight_duration_minutes: Option<u32>,
    /// Nightly accommodation price at the destination, or ``None``.
    pub accommodation_price: Option<i32>,
    /// Opaque booking token for constructing a deep link.
    pub booking_token: String,
}

#[pymethods]
impl ExploreResult {
    fn __repr__(&self) -> String {
        format!(
            "ExploreResult(name={:?}, airport={:?}, price={})",
            self.name,
            self.nearest_airport,
            repr_opt(&self.price),
        )
    }

    /// Return this destination as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("place_id", &self.place_id)?;
        d.set_item("name", &self.name)?;
        d.set_item("country", &self.country)?;
        d.set_item("lat", self.lat)?;
        d.set_item("lng", self.lng)?;
        d.set_item("image_url", &self.image_url)?;
        d.set_item("nearest_airport", &self.nearest_airport)?;
        d.set_item("flight_airport", &self.flight_airport)?;
        d.set_item("date_from", &self.date_from)?;
        d.set_item("date_to", &self.date_to)?;
        d.set_item("price", self.price)?;
        d.set_item("airline", &self.airline)?;
        d.set_item("stops", self.stops)?;
        d.set_item("flight_duration_minutes", self.flight_duration_minutes)?;
        d.set_item("accommodation_price", self.accommodation_price)?;
        d.set_item("booking_token", &self.booking_token)?;
        Ok(d)
    }
}

/// One discounted destination returned by :meth:`GFlights.deals`.
#[pyclass(get_all)]
#[derive(Clone, Debug)]
pub struct DealResult {
    /// Origin airport IATA code.
    pub origin_iata: String,
    /// Destination airport IATA code.
    pub destination_iata: String,
    /// Destination city name.
    pub destination_city: String,
    /// Destination country name.
    pub destination_country: String,
    /// Destination Google place MID, or ``None``.
    pub destination_mid: Option<String>,
    /// Outbound date as ``"YYYY-MM-DD"``, or ``None``.
    pub outbound_date: Option<String>,
    /// Return date as ``"YYYY-MM-DD"``, or ``None``.
    pub return_date: Option<String>,
    /// Deal price (round trip), or ``None``.
    pub price: Option<i32>,
    /// Typical price for this route, or ``None``.
    pub typical_price: Option<i32>,
    /// Percentage below typical price (e.g. ``68``), or ``None``.
    pub discount_pct: Option<i32>,
    /// Total flight duration in minutes, or ``None``.
    pub duration_minutes: Option<u32>,
    /// Number of stops (0 = non-stop), or ``None``.
    pub stops: Option<u8>,
    /// Operating airline code (``"*"`` for mixed), or ``None``.
    pub airline_code: Option<String>,
    /// Operating airline name, or ``None``.
    pub airline_name: Option<String>,
    /// Cover image URL, or ``None``.
    pub image_url: Option<String>,
    /// Short highlight phrases for the destination.
    pub highlights: Vec<String>,
    /// One-line description, or ``None``.
    pub description: Option<String>,
    /// Absolute Google Flights booking deep link, or ``None``.
    pub booking_url: Option<String>,
    /// Opaque booking token, or ``None``.
    pub booking_token: Option<String>,
}

#[pymethods]
impl DealResult {
    fn __repr__(&self) -> String {
        format!(
            "DealResult(dest={:?}, price={}, off={}%)",
            self.destination_city,
            repr_opt(&self.price),
            repr_opt(&self.discount_pct),
        )
    }

    /// Return this deal as a plain ``dict``.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("origin_iata", &self.origin_iata)?;
        d.set_item("destination_iata", &self.destination_iata)?;
        d.set_item("destination_city", &self.destination_city)?;
        d.set_item("destination_country", &self.destination_country)?;
        d.set_item("destination_mid", &self.destination_mid)?;
        d.set_item("outbound_date", &self.outbound_date)?;
        d.set_item("return_date", &self.return_date)?;
        d.set_item("price", self.price)?;
        d.set_item("typical_price", self.typical_price)?;
        d.set_item("discount_pct", self.discount_pct)?;
        d.set_item("duration_minutes", self.duration_minutes)?;
        d.set_item("stops", self.stops)?;
        d.set_item("airline_code", &self.airline_code)?;
        d.set_item("airline_name", &self.airline_name)?;
        d.set_item("image_url", &self.image_url)?;
        d.set_item("highlights", &self.highlights)?;
        d.set_item("description", &self.description)?;
        d.set_item("booking_url", &self.booking_url)?;
        d.set_item("booking_token", &self.booking_token)?;
        Ok(d)
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
    ///
    /// :param user_agent: Override the User-Agent header. By default a real
    ///                    desktop browser string is chosen from a rotating pool
    ///                    per client, so traffic is not trivially fingerprinted.
    /// :param proxy:      Route all requests through a proxy URL. Supports
    ///                    ``http://``, ``https://`` and ``socks5://``
    ///                    (e.g. ``"socks5://127.0.0.1:9050"``). ``None`` = direct.
    /// :param currency:   ISO-4217 currency code applied to every request
    ///                    (e.g. ``"USD"``, ``"EUR"``, ``"GBP"``). Default ``"EUR"``.
    /// :param lang:       BCP-47 language subtag applied to every request. Default ``"en"``.
    /// :param country:    ISO 3166-1 alpha-2 country code applied to every request. Default ``"GB"``.
    #[new]
    #[pyo3(signature = (user_agent = None, proxy = None, currency = "EUR", lang = "en", country = "GB"))]
    fn new(
        user_agent: Option<String>,
        proxy: Option<String>,
        currency: &str,
        lang: &str,
        country: &str,
    ) -> PyResult<Self> {
        let currency = parse_currency(currency)?;
        let rt = pyo3_async_runtimes::tokio::get_runtime();
        let mut client = match proxy {
            Some(p) => rt
                .block_on(ApiClient::new_with_proxy(p))
                .map_err(anyhow_to_py)?,
            None => rt.block_on(ApiClient::new()),
        };
        if let Some(ua) = user_agent {
            client = client.with_user_agent(ua);
        }
        client = client.with_locale(currency, lang, country);
        Ok(GFlights { client })
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
    /// :param max_price:    Maximum price cap (in the search currency). ``None`` for no cap.
    /// :param carry_on:     Number of carry-on bags required (0 = no restriction).
    /// :param checked_bags: Number of checked bags required (0 = no restriction).
    /// :param currency:     ISO-4217 currency code (e.g. ``"USD"``, ``"EUR"``, ``"GBP"``).
    /// :param lang:         BCP-47 language subtag (default ``"en"``).
    /// :param country:      ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:            Coroutine → ``list[FlightResult]``
    #[pyo3(signature = (
        from_airport,
        to_airport,
        date,
        return_date = None,
        adults = 1,
        children = 0,
        infants_in_seat = 0,
        infants_on_lap = 0,
        travel_class = "economy",
        stops = "all",
        sort = "best",
        airlines_include = vec![],
        airlines_exclude = vec![],
        via = vec![],
        lower_emissions = false,
        max_price = None,
        carry_on = 0,
        checked_bags = 0,
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
        children: u8,
        infants_in_seat: u8,
        infants_on_lap: u8,
        travel_class: &str,
        stops: &str,
        sort: &str,
        airlines_include: Vec<String>,
        airlines_exclude: Vec<String>,
        via: Vec<String>,
        lower_emissions: bool,
        max_price: Option<i32>,
        carry_on: u8,
        checked_bags: u8,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Validate synchronously — raises ValueError before the coroutine is even awaited.
        let dep_date = parse_date(&date)?;
        let ret_date = return_date.as_deref().map(parse_date).transpose()?;
        let stop_opt = parse_stop_options(stops)?;
        let class = parse_travel_class(travel_class)?;
        let sort_ord = parse_sort_order(sort)?;
        let inc = parse_airline_filters(&airlines_include)?;
        let exc = parse_airline_filters(&airlines_exclude)?;
        let travelers = gflights::parsers::common::Travelers::new(vec![
            adults.into(),
            children.into(),
            infants_on_lap.into(),
            infants_in_seat.into(),
        ])
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        // Convert &str → String before moving into the 'static async block.
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
                .travelers(travelers)
                .airlines_include(inc)
                .airlines_exclude(exc)
                .lower_emissions(lower_emissions);

            if let Some(p) = max_price {
                builder = builder.max_price(p);
            }
            if carry_on > 0 || checked_bags > 0 {
                builder = builder.baggage(carry_on, checked_bags);
            }
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
    /// :param currency:     ISO-4217 currency code (e.g. ``"EUR"``).
    /// :param lang:         BCP-47 language subtag.
    /// :param country:      ISO 3166-1 alpha-2 country code.
    /// :returns:            Coroutine → ``list[PriceEntry]``, sorted by date.
    #[pyo3(signature = (
        from_airport,
        to_airport,
        date,
        months = 1,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn price_graph<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        to_airport: String,
        date: String,
        months: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dep_date = parse_date(&date)?;
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
    /// :param currency:     ISO-4217 currency code (e.g. ``"EUR"``).
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
    ) -> PyResult<Bound<'py, PyAny>> {
        let dep_s = parse_date(&dep_start)?;
        let dep_e = parse_date(&dep_end)?;
        let ret_s = parse_date(&ret_start)?;
        let ret_e = parse_date(&ret_end)?;
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

    /// Search for flights across multiple legs (open-jaw / multi-city).
    ///
    /// :param legs:         List of ``(from_airport, to_airport, "YYYY-MM-DD")`` tuples.
    ///                      Minimum 2 legs.
    /// :param adults:       Number of adult passengers (default 1).
    /// :param travel_class: ``"economy"`` / ``"premium-economy"`` / ``"business"`` / ``"first"``.
    /// :param sort:         ``"best"`` / ``"price"`` / ``"duration"`` / etc.
    /// :param max_price:    Maximum price cap (in the search currency). ``None`` for no cap.
    /// :param carry_on:     Number of carry-on bags required (0 = no restriction).
    /// :param checked_bags: Number of checked bags required (0 = no restriction).
    /// :param currency:     ISO-4217 currency code (e.g. ``"EUR"``).
    /// :param lang:         BCP-47 language subtag (default ``"en"``).
    /// :param country:      ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:            Coroutine → ``list[FlightResult]``
    #[pyo3(signature = (
        legs,
        adults = 1,
        children = 0,
        infants_in_seat = 0,
        infants_on_lap = 0,
        travel_class = "economy",
        sort = "best",
        max_price = None,
        carry_on = 0,
        checked_bags = 0,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn multi_city_search<'py>(
        &self,
        py: Python<'py>,
        legs: Vec<(String, String, String)>,
        adults: u8,
        children: u8,
        infants_in_seat: u8,
        infants_on_lap: u8,
        travel_class: &str,
        sort: &str,
        max_price: Option<i32>,
        carry_on: u8,
        checked_bags: u8,
    ) -> PyResult<Bound<'py, PyAny>> {
        if legs.len() < 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "multi_city_search requires at least 2 legs",
            ));
        }
        let parsed_legs: Vec<(String, String, chrono::NaiveDate)> = legs
            .iter()
            .map(|(from, to, date)| {
                let d = parse_date(date)?;
                Ok((from.clone(), to.clone(), d))
            })
            .collect::<PyResult<_>>()?;

        let class = parse_travel_class(travel_class)?;
        let sort_ord = parse_sort_order(sort)?;
        let travelers = gflights::parsers::common::Travelers::new(vec![
            adults.into(),
            children.into(),
            infants_on_lap.into(),
            infants_in_seat.into(),
        ])
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut builder = MultiCityConfig::builder()
                .travellers(travelers)
                .travel_class(class)
                .sort_order(sort_ord);

            if let Some(p) = max_price {
                builder = builder.max_price(p);
            }
            if carry_on > 0 || checked_bags > 0 {
                builder = builder.baggage(carry_on, checked_bags);
            }

            for (from, to, date) in &parsed_legs {
                builder = builder
                    .add_leg(from, to, *date, &client)
                    .await
                    .map_err(anyhow_to_py)?;
            }

            let config = builder.build().map_err(anyhow_to_py)?;
            let flights = client
                .request_multi_city_flights(&config)
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

    /// Explore cheap destinations from an origin airport (Google Flights Explore mode).
    ///
    /// :param from_airport:       Origin IATA code (e.g. ``"LUX"``).
    /// :param month:              Calendar month (1–12) to search in.  ``None`` for any month.
    /// :param duration:           Trip duration: ``"weekend"``, ``"week"``, or ``"2weeks"``.
    /// :param max_price:          Maximum total round-trip price.  ``None`` for no limit.
    /// :param interest:           Interest name (e.g. ``"beaches"``, ``"climbing"``), an alias,
    ///                            or a raw ``/m/…`` MID. Unknown values raise ``ValueError``.
    /// :param max_flight_hours:   Maximum one-way flight time in hours.  ``None`` for no limit.
    /// :param carry_on:           Number of carry-on bags (default 0).
    /// :param checked:            Number of checked bags (default 0).
    /// :param adults:             Number of adult passengers (default 1).
    /// :param travel_class:       ``"economy"`` / ``"premium-economy"`` / ``"business"`` / ``"first"``.
    /// :param currency:           ISO-4217 currency code (e.g. ``"EUR"``).
    /// :param lang:               BCP-47 language subtag (default ``"en"``).
    /// :param country:            ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:                  Coroutine → ``list[ExploreResult]``
    #[pyo3(signature = (
        from_airport,
        month = None,
        duration = "week",
        max_price = None,
        interest = None,
        max_flight_hours = None,
        carry_on = 0u8,
        checked = 0u8,
        adults = 1,
        children = 0,
        infants_in_seat = 0,
        infants_on_lap = 0,
        travel_class = "economy",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn explore<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        month: Option<u8>,
        duration: &str,
        max_price: Option<i32>,
        interest: Option<String>,
        max_flight_hours: Option<u32>,
        carry_on: u8,
        checked: u8,
        adults: u8,
        children: u8,
        infants_in_seat: u8,
        infants_on_lap: u8,
        travel_class: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let trip_duration = match duration.to_lowercase().as_str() {
            "weekend" => ExploreDuration::Weekend,
            "week" | "1week" | "one-week" => ExploreDuration::OneWeek,
            "2weeks" | "two-weeks" | "twoweeks" => ExploreDuration::TwoWeeks,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown duration {other:?} — use 'weekend', 'week', or '2weeks'"
                )));
            }
        };

        let class = parse_travel_class(travel_class)?;
        let travelers = gflights::parsers::common::Travelers::new(vec![
            adults.into(),
            children.into(),
            infants_on_lap.into(),
            infants_in_seat.into(),
        ])
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let max_flight_duration_minutes = max_flight_hours.map(|h| h * 60);
        let baggage = if carry_on > 0 || checked > 0 {
            Some((carry_on, checked))
        } else {
            None
        };

        // Resolve an interest name/alias (e.g. "beaches") or raw MID to a MID,
        // raising on unknown values instead of silently returning no results.
        let interest = interest
            .map(|i| gflights::requests::config::explore::resolve_interest(&i))
            .transpose()
            .map_err(anyhow_to_py)?;

        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Resolve origin airport/city to a Location via the city-lookup endpoint.
            let origin_loc =
                if from_airport.len() == 3 && from_airport.chars().all(char::is_uppercase) {
                    gflights::parsers::common::Location {
                        loc_identifier: from_airport.clone(),
                        loc_type: gflights::parsers::common::PlaceType::Airport,
                        location_name: Some(from_airport.clone()),
                    }
                } else {
                    client
                        .request_city(&from_airport)
                        .await
                        .map_err(anyhow_to_py)?
                        .to_city_list()
                };

            let config = ExploreConfig {
                origin: vec![origin_loc],
                destination: None,
                trip_date: month.map(|m| ExploreDate { month: m }),
                trip_duration,
                max_price,
                interest,
                airline_alliance: None,
                max_flight_duration_minutes,
                baggage,
                map_bounds: None,
                travellers: travelers,
                travel_class: class,
            };

            let results = client
                .request_explore(&config)
                .await
                .map_err(anyhow_to_py)?;

            Python::with_gil(|py| {
                results
                    .into_iter()
                    .map(|r| {
                        Py::new(
                            py,
                            ExploreResult {
                                place_id: r.place_id,
                                name: r.name,
                                country: r.country,
                                lat: r.coords.0,
                                lng: r.coords.1,
                                image_url: r.image_url,
                                nearest_airport: r.nearest_airport,
                                flight_airport: r.flight_airport,
                                date_from: r.date_from.map(|d| d.to_string()),
                                date_to: r.date_to.map(|d| d.to_string()),
                                price: r.price,
                                airline: r.airline,
                                stops: r.stops,
                                flight_duration_minutes: r.flight_duration_minutes,
                                accommodation_price: r.accommodation_price,
                                booking_token: r.booking_token,
                            },
                        )
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
        })
    }

    /// Find discounted destinations (flight deals) from an origin.
    ///
    /// The ``out`` / ``ret`` pair acts as a trip-length anchor; the endpoint
    /// returns deals of similar length across many dates.
    ///
    /// :param from_airport:  Origin IATA code or city name.
    /// :param out:           Outbound date ``"YYYY-MM-DD"`` (trip-length anchor).
    /// :param ret:           Return date ``"YYYY-MM-DD"``.
    /// :param nonstop:       Only non-stop deals (default ``False``).
    /// :param max_hours:     Maximum one-way flight time in hours. ``None`` = no limit.
    /// :param adults:        Number of adult passengers (default 1).
    /// :param travel_class:  ``"economy"`` / ``"premium-economy"`` / ``"business"`` / ``"first"``.
    /// :param currency:      ISO-4217 currency code (e.g. ``"EUR"``).
    /// :param lang:          BCP-47 language subtag (default ``"en"``).
    /// :param country:       ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:             Coroutine → ``list[DealResult]``
    #[pyo3(signature = (
        from_airport,
        out,
        ret,
        nonstop = false,
        max_hours = None,
        adults = 1,
        children = 0,
        infants_in_seat = 0,
        infants_on_lap = 0,
        travel_class = "economy",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn deals<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        out: &str,
        ret: &str,
        nonstop: bool,
        max_hours: Option<u32>,
        adults: u8,
        children: u8,
        infants_in_seat: u8,
        infants_on_lap: u8,
        travel_class: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let outbound_date = parse_date(out)?;
        let return_date = parse_date(ret)?;
        let class = parse_travel_class(travel_class)?;
        let travelers = gflights::parsers::common::Travelers::new(vec![
            adults.into(),
            children.into(),
            infants_on_lap.into(),
            infants_in_seat.into(),
        ])
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let origin_loc =
                if from_airport.len() == 3 && from_airport.chars().all(char::is_uppercase) {
                    gflights::parsers::common::Location {
                        loc_identifier: from_airport.clone(),
                        loc_type: gflights::parsers::common::PlaceType::Airport,
                        location_name: Some(from_airport.clone()),
                    }
                } else {
                    client
                        .request_city(&from_airport)
                        .await
                        .map_err(anyhow_to_py)?
                        .to_city_list()
                };

            let config = DealConfig {
                origin: vec![origin_loc],
                outbound_date,
                return_date,
                nonstop,
                max_duration_minutes: max_hours.map(|h| h * 60),
                travel_class: class,
                travellers: travelers,
            };

            let results = client.request_deals(&config).await.map_err(anyhow_to_py)?;

            Python::with_gil(|py| {
                results
                    .into_iter()
                    .map(|r| {
                        Py::new(
                            py,
                            DealResult {
                                origin_iata: r.origin_iata,
                                destination_iata: r.destination_iata,
                                destination_city: r.destination_city,
                                destination_country: r.destination_country,
                                destination_mid: r.destination_mid,
                                outbound_date: r.outbound_date.map(|d| d.to_string()),
                                return_date: r.return_date.map(|d| d.to_string()),
                                price: r.price,
                                typical_price: r.typical_price,
                                discount_pct: r.discount_pct,
                                duration_minutes: r.duration_minutes,
                                stops: r.stops,
                                airline_code: r.airline_code,
                                airline_name: r.airline_name,
                                image_url: r.image_url,
                                highlights: r.highlights,
                                description: r.description,
                                booking_url: r.booking_url,
                                booking_token: r.booking_token,
                            },
                        )
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
        })
    }

    /// Find the cheapest departure dates for a route over a range of months.
    ///
    /// :param from_airport:       Origin IATA code or city name.
    /// :param to_airport:         Destination IATA code or city name.
    /// :param date:               Start of the search window as ``"YYYY-MM-DD"``.
    /// :param months:             Number of months to scan. Default ``3``.
    /// :param trip_duration_days: Round-trip length in days. ``None`` for one-way
    ///                            date discovery.
    /// :param currency:           ISO-4217 currency code (default ``"EUR"``).
    /// :param lang:               BCP-47 language subtag (default ``"en"``).
    /// :param country:            ISO 3166-1 alpha-2 country code (default ``"GB"``).
    /// :returns:                  Coroutine → ``list[CheapDate]``, sorted cheapest first.
    #[pyo3(signature = (
        from_airport,
        to_airport,
        date,
        months = 3,
        trip_duration_days = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn cheapest_dates<'py>(
        &self,
        py: Python<'py>,
        from_airport: String,
        to_airport: String,
        date: String,
        months: u32,
        trip_duration_days: Option<u32>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dep_start = parse_date(&date)?;
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let config = Config::builder()
                .departure(&from_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .destination(&to_airport, &client)
                .await
                .map_err(anyhow_to_py)?
                .departing_date(dep_start)
                .build()
                .map_err(anyhow_to_py)?;

            let results = client
                .cheapest_dates(&config, chrono::Months::new(months), trip_duration_days)
                .await
                .map_err(anyhow_to_py)?;

            Python::with_gil(|py| {
                results
                    .into_iter()
                    .map(|r| {
                        Py::new(
                            py,
                            CheapDate {
                                departure_date: r.departure_date.to_string(),
                                return_date: r.return_date.map(|d| d.to_string()),
                                price: r.price,
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
    m.add_class::<CheapDate>()?;
    m.add_class::<ExploreResult>()?;
    m.add_class::<DealResult>()?;
    m.add("GFlightsError", m.py().get_type::<GFlightsError>())?;
    Ok(())
}
