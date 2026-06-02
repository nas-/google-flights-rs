use super::config::Currency;
use crate::parsers;
use crate::parsers::constants::{CLK_URL, FLIGHTS_MAIN_PAGE};
use crate::parsers::common::FixedFlights;
use crate::requests::config::{Config, MultiCityConfig, TripType};
use anyhow::Result;
use chrono::{Duration, Months, NaiveDate};
use governor::{DefaultDirectRateLimiter, Quota};
use parsers::calendar_graph_request::GraphRequestOptions;
use parsers::calendar_graph_response::GraphRawResponseContainer;
use parsers::city_request::CityRequestOptions;
use parsers::city_response::ResponseInnerBodyParsed;
use parsers::common::ToRequestBody;
use parsers::date_grid_request::{DateGridRequestOptions, DATE_GRID_MAX_CELLS};
use parsers::date_grid_response::{parse_date_grid_response, CheapDate, DateGridResponse};
use parsers::flight_request::{FlightRequestOptions, MultiCityRequestOptions};
use parsers::flight_response::{create_raw_response_vec, FlightResponseContainer};
use parsers::offer_response::{self, OfferRawResponseContainer};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response, StatusCode};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Returned when the Google Flights API responds with HTTP 429 (Too Many Requests).
///
/// Once any request on an [`ApiClient`] receives a 429, that client (and all
/// its clones, which share the same flag) will refuse to send further requests
/// until [`ApiClient::reset_rate_limit`] is called.
///
/// You can match on this error type via [`anyhow::Error::downcast_ref`]:
///
/// ```rust,ignore
/// if let Some(_) = err.downcast_ref::<RateLimitedError>() {
///     // wait and retry, or surface to the user
/// }
/// ```
#[derive(Debug)]
pub struct RateLimitedError;

impl std::fmt::Display for RateLimitedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Google Flights returned HTTP 429 Too Many Requests — all further requests on this client are blocked; call ApiClient::reset_rate_limit() to resume")
    }
}

impl std::error::Error for RateLimitedError {}

/// Configuration for automatic retry with exponential back-off.
///
/// Applied to transient server errors (HTTP 500/502/503/504) and timed-out
/// connections.  429s and 4xx client errors are never retried.
///
/// # Example
/// ```rust
/// use gflights::requests::api::RetryConfig;
/// let cfg = RetryConfig { max_attempts: 5, base_delay_ms: 200, cap_delay_ms: 10_000 };
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Total number of attempts (including the first).  `1` means no retries.
    pub max_attempts: u32,
    /// Base delay in milliseconds before the first retry.  Doubles each attempt.
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds (before jitter).
    pub cap_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            cap_delay_ms: 30_000,
        }
    }
}

/// The `ApiClient` struct is used to send requests to the Google Flights website.
///
/// Cloning this struct is cheap — all clones share the same underlying HTTP
/// client, rate-limiter, and rate-limit flag via `Arc`.
#[derive(Clone)]
pub struct ApiClient {
    pub rate_limiter: Arc<DefaultDirectRateLimiter>,
    pub client: Arc<Client>,
    frontend_version: String,
    /// Set to `true` the first time any request on this client (or any clone)
    /// receives HTTP 429.  While `true`, every call to `do_request` returns
    /// [`RateLimitedError`] immediately without touching the network.
    rate_limited: Arc<AtomicBool>,
    /// Retry policy for transient server errors and timeouts.
    retry_config: RetryConfig,
}

impl ApiClient {
    /// Creates a new instance of `ApiClient` with a default rate limiter of 10 requests per second.
    pub async fn new() -> Self {
        // NonZeroU32::MIN is 1; saturating_add(9) gives 10 with no possibility of panic.
        let rate_limiter_quota = Quota::per_second(NonZeroU32::MIN.saturating_add(9));
        Self::new_with_ratelimit(rate_limiter_quota).await
    }

    /// Creates a new instance of `ApiClient` with a custom rate limiter.
    pub async fn new_with_ratelimit(rate_limiter_quota: Quota) -> Self {
        let rate_limiter: Arc<DefaultDirectRateLimiter> =
            Arc::new(DefaultDirectRateLimiter::direct(rate_limiter_quota));
        let frontend_version = get_frontend_version().await;

        Self {
            rate_limiter,
            client: Arc::new(Client::new()),
            frontend_version: frontend_version
                .unwrap_or("boq_travel-frontend-flights-ui_20260527.01_p0".into()),
            rate_limited: Arc::new(AtomicBool::new(false)),
            retry_config: RetryConfig::default(),
        }
    }

    /// Overrides the retry policy for this client.
    ///
    /// ```rust
    /// # use gflights::requests::api::{ApiClient, RetryConfig};
    /// # async fn example() {
    /// let client = ApiClient::new().await
    ///     .with_retry_config(RetryConfig { max_attempts: 5, base_delay_ms: 200, cap_delay_ms: 10_000 });
    /// # }
    /// ```
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Returns `true` if this client has been halted by a 429 response.
    ///
    /// All clones of the same `ApiClient` share this flag.
    pub fn is_rate_limited(&self) -> bool {
        self.rate_limited.load(Ordering::SeqCst)
    }

    /// Clears the 429 flag so the client can send requests again.
    ///
    /// Call this after an appropriate back-off period.  The client will resume
    /// normal operation on the next request.
    pub fn reset_rate_limit(&self) {
        self.rate_limited.store(false, Ordering::SeqCst);
    }

    /// Sends a request to retrieve information about a city/airport.
    ///
    /// # Arguments
    ///
    /// * `city` - The name of the city, in english
    ///
    /// # Returns
    ///
    /// Returns a `ResponseInnerBodyParsed` object containing the parsed response.
    /// This will contains both the airport associated and the city.
    #[tracing::instrument(skip(self))]
    pub async fn request_city(&self, city: &str) -> Result<ResponseInnerBodyParsed> {
        let options = CityRequestOptions {
            city: city.to_owned(),
            frontend_version: self.frontend_version.clone(),
        };
        let city_response: &str = &self
            .do_request(&options, None, "en", "GB")
            .await?
            .text()
            .await?;
        let cities_res = ResponseInnerBodyParsed::try_from(city_response)?;
        Ok(cities_res)
    }

    /// Sends a request to retrieve flight graph data.
    ///
    /// # Arguments
    ///
    /// * `args` - The configuration options for the request.
    /// * `months` - The number of months to include in the graph.
    ///
    /// # Returns
    ///
    /// Returns a `GraphRawResponseContainer` object containing the parsed response.
    #[tracing::instrument(skip_all)]
    pub async fn request_graph(
        &self,
        args: &Config,
        months: Months,
    ) -> Result<GraphRawResponseContainer> {
        let date_end_graph = args
            .get_end_graph(months)
            .ok_or_else(|| anyhow::anyhow!("date overflow when computing graph end date"))?
            .to_string();
        let req_options = GraphRequestOptions {
            departing_city: &args.departure,
            arriving_city: &args.destination,
            date_start: &args.departing_date,
            date_return: args.return_date.as_ref(),
            date_end_graph: &date_end_graph,
            travellers: args.travellers.clone(),
            travel_class: &args.travel_class,
            stop_option: &args.stop_options,
            departing_times: &args.departing_times,
            return_times: &args.return_times,
            stopover_max: &args.stopover_max,
            stopover_min: &args.stopover_min,
            duration_max: &args.duration_max,
            frontend_version: &self.frontend_version,
            language: &args.language,
            country: &args.country,
            sort_order: &args.sort_order,
        };
        let body = self
            .do_request(
                &req_options,
                Some(args.currency.clone()),
                &args.language,
                &args.country,
            )
            .await?
            .text()
            .await?;
        GraphRawResponseContainer::try_from(body.as_ref())
    }

    /// Sends a request to retrieve the date-grid price matrix.
    ///
    /// Returns a price for every (departure_date, return_date) combination
    /// that falls within the two supplied date windows.
    ///
    /// The backend rejects requests whose cell count
    /// (`dep_window_days × ret_window_days`) exceeds [`DATE_GRID_MAX_CELLS`]
    /// (200).  This method transparently splits large windows into multiple
    /// sub-requests and merges the results, so callers are free to supply any
    /// window size.
    ///
    /// # Arguments
    ///
    /// * `args` — Config supplying route, travellers, cabin class, etc.
    ///   `args.departing_date` / `args.return_date` are used as reference
    ///   dates inside the itinerary body; they must fall within the respective
    ///   windows and a return date must be set.
    /// * `dep_start` / `dep_end` — window of candidate departure dates.
    /// * `ret_start` / `ret_end` — window of candidate return dates.
    #[tracing::instrument(skip_all)]
    pub async fn request_date_grid(
        &self,
        args: &Config,
        dep_start: NaiveDate,
        dep_end: NaiveDate,
        ret_start: NaiveDate,
        ret_end: NaiveDate,
    ) -> Result<DateGridResponse> {
        args.return_date
            .ok_or_else(|| anyhow::anyhow!("date grid requires a return date in Config"))?;

        let dep_days = (dep_end - dep_start).num_days() + 1;
        let ret_days = (ret_end - ret_start).num_days() + 1;
        let total_cells = dep_days * ret_days;

        if total_cells <= DATE_GRID_MAX_CELLS as i64 {
            // Fits in a single request.
            return self
                .request_date_grid_chunk(args, dep_start, dep_end, ret_start, ret_end)
                .await;
        }

        // Split: keep the full departure window and slice the return window
        // into chunks whose cell count stays within the limit.
        // If the departure window alone already exceeds the limit, each chunk
        // covers exactly one return day (best we can do).
        let max_ret_chunk = ((DATE_GRID_MAX_CELLS as i64) / dep_days).max(1);
        tracing::info!(
            dep_days,
            ret_days,
            max_ret_chunk,
            "date grid too large, splitting into chunks"
        );

        let mut all_entries = Vec::new();
        let mut chunk_ret_start = ret_start;

        while chunk_ret_start <= ret_end {
            let chunk_ret_end = (chunk_ret_start + Duration::days(max_ret_chunk - 1)).min(ret_end);

            let chunk = self
                .request_date_grid_chunk(args, dep_start, dep_end, chunk_ret_start, chunk_ret_end)
                .await?;
            all_entries.extend(chunk.entries);

            chunk_ret_start = chunk_ret_end + Duration::days(1);
        }

        Ok(DateGridResponse {
            entries: all_entries,
        })
    }

    /// Single `GetCalendarGrid` request — windows must be ≤ [`DATE_GRID_MAX_CELLS`] cells.
    ///
    /// The return reference date is clamped to `[ret_start, ret_end]` so it
    /// stays valid when this is called from the chunking loop.
    async fn request_date_grid_chunk(
        &self,
        args: &Config,
        dep_start: NaiveDate,
        dep_end: NaiveDate,
        ret_start: NaiveDate,
        ret_end: NaiveDate,
    ) -> Result<DateGridResponse> {
        // Clamp the config's reference dates to lie within the supplied windows.
        let dep_ref = args.departing_date.max(dep_start).min(dep_end);
        let ret_ref = args
            .return_date
            .unwrap_or(ret_start)
            .max(ret_start)
            .min(ret_end);

        let req_options = DateGridRequestOptions::new(
            &args.departure,
            &args.destination,
            &dep_ref,
            &ret_ref,
            &dep_start,
            &dep_end,
            &ret_start,
            &ret_end,
            args.travellers.clone(),
            &args.travel_class,
            &args.stop_options,
            &args.departing_times,
            &args.return_times,
            &args.stopover_max,
            &args.duration_max,
            &self.frontend_version,
        );

        let body = self
            .do_request(
                &req_options,
                Some(args.currency.clone()),
                &args.language,
                &args.country,
            )
            .await?
            .text()
            .await?;
        parse_date_grid_response(&body)
    }

    /// Sends a request to retrieve flight data.
    ///
    /// # Arguments
    ///
    /// * `args` - The configuration options for the request.
    ///
    /// # Returns
    ///
    /// Returns a `FlightResponseContainer` object containing the parsed response.
    #[tracing::instrument(skip_all, fields(
        from = ?args.departure.iter().map(|l| l.loc_identifier.as_str()).collect::<Vec<_>>(),
        to = ?args.destination.iter().map(|l| l.loc_identifier.as_str()).collect::<Vec<_>>(),
        date = %args.departing_date,
        class = ?args.travel_class,
        stops = ?args.stop_options,
    ))]
    pub async fn request_flights(&self, args: &Config) -> Result<FlightResponseContainer> {
        tracing::info!("Requesting flights");
        let body = self.fetch_flight_body(args).await?;
        create_raw_response_vec(body)
    }

    /// Sends a request to retrieve flight offer data.
    ///
    /// # Arguments
    ///
    /// * `args` - The configuration options for the request.
    ///
    /// # Returns
    ///
    /// Returns an `OfferRawResponseContainer` object containing the parsed response.
    #[tracing::instrument(skip_all, fields(
        from = ?args.departure.iter().map(|l| l.loc_identifier.as_str()).collect::<Vec<_>>(),
        to = ?args.destination.iter().map(|l| l.loc_identifier.as_str()).collect::<Vec<_>>(),
        date = %args.departing_date,
        class = ?args.travel_class,
        stops = ?args.stop_options,
    ))]
    pub async fn request_offer(&self, args: &Config) -> Result<OfferRawResponseContainer> {
        tracing::info!("Requesting offers");
        let body = self.fetch_flight_body(args).await?;
        tracing::trace!(body = %body, "raw offer response body");
        offer_response::create_raw_response_offer_vec(body)
    }

    /// Builds the request options from a [`Config`] and POSTs to the flights endpoint,
    /// returning the raw response body.
    ///
    /// Shared by [`Self::request_flights`] and [`Self::request_offer`], which differ only
    /// in how they parse the body.
    async fn fetch_flight_body(&self, args: &Config) -> Result<String> {
        let date_start = args.departing_date.to_string();
        let date_return = args.return_date.map(|f| f.to_string());
        // DepartureTime/ArrivalTime are client-side-only sorts; the backend does
        // not accept those discriminants and returns an empty result if sent.
        let server_sort = args.sort_order.server_sort();
        let req_options = FlightRequestOptions {
            departing_city: &args.departure,
            arriving_city: &args.destination,
            date_start: &date_start,
            date_return: date_return.as_deref(),
            travellers: args.travellers.clone(),
            travel_class: &args.travel_class,
            stop_option: &args.stop_options,
            departing_times: &args.departing_times,
            return_times: &args.return_times,
            stopover_max: &args.stopover_max,
            stopover_min: &args.stopover_min,
            duration_max: &args.duration_max,
            frontend_version: &self.frontend_version,
            fixed_flights: &args.fixed_flights,
            language: &args.language,
            country: &args.country,
            sort_order: &server_sort,
            airlines_include: &args.airlines_include,
            airlines_exclude: &args.airlines_exclude,
            connecting_airports: &args.connecting_airports,
            lower_emissions: args.lower_emissions,
        };
        Ok(self
            .do_request(
                &req_options,
                Some(args.currency.clone()),
                &args.language,
                &args.country,
            )
            .await?
            .text()
            .await?)
    }

    /// Sends a multi-city (open-jaw) flight search request.
    ///
    /// Returns all flight options across all legs in a single
    /// [`FlightResponseContainer`].  Call [`FlightResponseContainer::get_all_flights`]
    /// to obtain the deduplicated itinerary list.
    ///
    /// # Arguments
    ///
    /// * `args` — [`MultiCityConfig`] built via [`MultiCityConfig::builder()`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gflights::requests::{api::ApiClient, config::MultiCityConfig};
    /// use chrono::NaiveDate;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let client = ApiClient::new().await;
    ///     let config = MultiCityConfig::builder()
    ///         .add_leg("LUX", "FCO", NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(), &client).await?
    ///         .add_leg("FCO", "MAD", NaiveDate::from_ymd_opt(2026, 9, 13).unwrap(), &client).await?
    ///         .add_leg("MAD", "LUX", NaiveDate::from_ymd_opt(2026, 9, 17).unwrap(), &client).await?
    ///         .build()?;
    ///     let results = client.request_multi_city_flights(&config).await?;
    ///     let flights = results.get_all_flights();
    ///     println!("{} itineraries found", flights.len());
    ///     Ok(())
    /// }
    /// ```
    #[tracing::instrument(skip_all, fields(
        legs = args.legs.len(),
        class = ?args.travel_class,
    ))]
    pub async fn request_multi_city_flights(
        &self,
        args: &MultiCityConfig,
    ) -> Result<FlightResponseContainer> {
        tracing::info!("Requesting multi-city flights");
        let req_options = MultiCityRequestOptions {
            config: args,
            frontend_version: &self.frontend_version,
        };
        let body = self
            .do_request(
                &req_options,
                Some(args.currency.clone()),
                &args.language,
                &args.country,
            )
            .await?
            .text()
            .await?;
        create_raw_response_vec(body)
    }

    /// Resolves a `click_token` from an `OfferGroup` or `BookingSubOption`
    /// into the final airline / OTA booking URL.
    ///
    /// Internally this POSTs the token to Google's click-tracker endpoint
    /// (`/travel/clk/f`) and extracts the redirect URL from the HTML
    /// `<meta http-equiv="refresh">` response.
    ///
    /// Find the cheapest departure dates for a route over a date range.
    ///
    /// * `trip_duration_days = None` — one-way mode: scans the price calendar
    ///   over `months` from `config.departing_date` and returns dates sorted by
    ///   price ascending.
    /// * `trip_duration_days = Some(n)` — round-trip mode: queries the date
    ///   grid for all departure dates in the range and returns only
    ///   `(dep, dep + n)` pairs, sorted by price ascending.
    ///
    /// # Example
    /// ```no_run
    /// # tokio_test::block_on(async {
    /// use gflights::requests::api::ApiClient;
    /// use gflights::parsers::common::{Location, PlaceType};
    /// use gflights::requests::config::Config;
    /// use chrono::{Months, NaiveDate};
    ///
    /// let client = ApiClient::new();
    /// let config = Config::builder()
    ///     .departure_location(Location {
    ///         loc_identifier: "LUX".into(),
    ///         loc_type: PlaceType::Airport,
    ///         location_name: None,
    ///     })
    ///     .destination_location(Location {
    ///         loc_identifier: "JFK".into(),
    ///         loc_type: PlaceType::Airport,
    ///         location_name: None,
    ///     })
    ///     .departing_date(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
    ///     .build()
    ///     .unwrap();
    ///
    /// // Cheapest one-way days over the next 3 months
    /// let oneway = client.cheapest_dates(&config, Months::new(3), None).await.unwrap();
    ///
    /// // Cheapest 7-night round trips over the next 3 months
    /// let roundtrip = client.cheapest_dates(&config, Months::new(3), Some(7)).await.unwrap();
    /// # });
    /// ```
    #[tracing::instrument(skip_all)]
    pub async fn cheapest_dates(
        &self,
        config: &Config,
        months: Months,
        trip_duration_days: Option<u32>,
    ) -> Result<Vec<CheapDate>> {
        match trip_duration_days {
            None => {
                let graph = self.request_graph(config, months).await?;
                let mut results: Vec<CheapDate> = graph
                    .get_all_graphs()
                    .into_iter()
                    .filter_map(|e| {
                        e.proposed_trip_cost.as_ref().map(|c| CheapDate {
                            departure_date: e.proposed_departure_date,
                            return_date: e.proposed_return_date,
                            price: c.trip_cost.price,
                        })
                    })
                    .collect();
                results.sort_by_key(|e| e.price);
                Ok(results)
            }
            Some(n) => {
                let dep_start = config.departing_date;
                let n_duration = Duration::days(i64::from(n));
                let dep_end = dep_start + months;
                let ret_start = dep_start + n_duration;
                let ret_end = dep_end + n_duration;

                // request_date_grid requires a round-trip config with return_date set.
                let rt_config = Config {
                    return_date: Some(dep_start + n_duration),
                    trip_type: TripType::Return,
                    fixed_flights: FixedFlights::new(2),
                    ..config.clone()
                };

                let grid = self
                    .request_date_grid(&rt_config, dep_start, dep_end, ret_start, ret_end)
                    .await?;

                let mut results: Vec<CheapDate> = grid
                    .entries
                    .into_iter()
                    .filter(|e| e.return_date - e.departure_date == n_duration)
                    .map(|e| CheapDate {
                        departure_date: e.departure_date,
                        return_date: Some(e.return_date),
                        price: e.price,
                    })
                    .collect();
                results.sort_by_key(|e| e.price);
                Ok(results)
            }
        }
    }

    /// # Example
    /// ```no_run
    /// # async fn example(client: gflights::requests::api::ApiClient, token: &str) {
    /// let url = client.resolve_booking_url(token).await.unwrap();
    /// println!("Book here: {url}");
    /// # }
    /// ```
    #[tracing::instrument(skip_all)]
    pub async fn resolve_booking_url(&self, click_token: &str) -> Result<String> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let t = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let url = format!("{CLK_URL}?t={t}");
        // The token is URL-safe base64 so no extra percent-encoding is needed,
        // but we send it as a form body value.
        let body = format!("u={click_token}");

        tracing::debug!(%url, "resolving booking URL");

        let html = self
            .client
            .post(&url)
            .body(body)
            .headers(get_headers(None, "en", "GB")?)
            .send()
            .await?
            .text()
            .await?;

        // Response is: <meta content="0;url='https://...'" http-equiv="refresh">
        // Handle both single-quoted and double-quoted url values.
        let re = Regex::new(r#"(?i)url=['"]([^'"]+)['"]"#).unwrap();
        let raw = re
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| anyhow::anyhow!("no redirect URL found in clk/f response"))?;

        // The URL is embedded in HTML, so & is encoded as &amp; — decode it.
        Ok(raw.replace("&amp;", "&"))
    }

    /// Sends a single HTTP request, enforcing the shared rate-limit flag and
    /// retrying on transient server errors according to [`RetryConfig`].
    ///
    /// Returns [`RateLimitedError`] (as an `anyhow::Error`) in two situations:
    /// - The flag is already set from a previous 429 on this client or any clone.
    /// - The server responds with HTTP 429 (the flag is then set for all clones).
    ///
    /// Retries (up to `retry_config.max_attempts - 1` times) are performed for:
    /// - HTTP 500, 502, 503, 504
    /// - Connection timeouts (`reqwest::Error::is_timeout()`)
    ///
    /// 4xx errors (other than 429) are not retried.
    #[tracing::instrument(skip_all)]
    async fn do_request(
        &self,
        options: &impl ToRequestBody,
        currency: Option<Currency>,
        language: &str,
        country: &str,
    ) -> Result<Response> {
        // Refuse immediately if a previous request already received a 429.
        if self.rate_limited.load(Ordering::SeqCst) {
            return Err(anyhow::Error::new(RateLimitedError));
        }

        let req_payload = options.to_request_body()?;
        let headers = get_headers(currency, language, country)?;

        let decoded_body = percent_encoding::percent_decode_str(&req_payload.body)
            .decode_utf8_lossy()
            .into_owned();
        tracing::trace!(
            url = %req_payload.url,
            body = %decoded_body,
            ?headers,
            "Outgoing POST request"
        );

        let max_attempts = self.retry_config.max_attempts.max(1);
        let base_delay = self.retry_config.base_delay_ms;
        let cap_delay = self.retry_config.cap_delay_ms;

        // `last_err` is only read when the loop is exhausted (attempt == max_attempts - 1).
        let mut last_err: anyhow::Error = anyhow::anyhow!("all retry attempts exhausted");

        for attempt in 0..max_attempts {
            if attempt > 0 {
                // Exponential back-off: base * 2^(attempt-1), capped, plus deterministic
                // jitter derived from the attempt number (no `rand` dependency needed).
                let backoff = (base_delay * (1u64 << (attempt - 1).min(30))).min(cap_delay);
                let jitter = (attempt as u64 * 37) % 101;
                let delay_ms = backoff + jitter;
                tracing::debug!(
                    attempt,
                    delay_ms,
                    "transient error — retrying after back-off"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            // Consume one rate-limiter slot per attempt.
            let _permit = self
                .rate_limiter
                .until_n_ready(NonZeroU32::MIN) // MIN == 1
                .await;

            let res = match self
                .client
                .post(req_payload.url.as_str())
                .body(req_payload.body.clone())
                .headers(headers.clone())
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) if e.is_timeout() => {
                    tracing::warn!(attempt, error = %e, "request timed out");
                    last_err = e.into();
                    continue; // retry
                }
                Err(e) => return Err(e.into()), // non-transient network error
            };

            tracing::trace!(
                status = %res.status(),
                http_version = ?res.version(),
                "Response received"
            );

            match res.status() {
                StatusCode::OK => return Ok(res),
                StatusCode::TOO_MANY_REQUESTS => {
                    // Signal all clones to stop; they will return RateLimitedError
                    // on their next attempt without hitting the network.
                    self.rate_limited.store(true, Ordering::SeqCst);
                    return Err(anyhow::Error::new(RateLimitedError));
                }
                StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT => {
                    tracing::warn!(
                        attempt,
                        status = %res.status(),
                        "server error — will retry if attempts remain"
                    );
                    last_err = anyhow::anyhow!("server error: {}", res.status());
                    // continue to next attempt
                }
                status => {
                    tracing::warn!(
                        http_version = ?res.version(),
                        status_code = %status,
                        "Unexpected HTTP response status"
                    );
                    return Ok(res);
                }
            }
        }

        Err(last_err)
    }
}

/// Static base headers shared by all requests.
///
/// Note: `x-goog-batchexecute-bgr` is intentionally omitted — its value is
/// difficult to reverse-engineer and its absence only slightly reduces result
/// accuracy.
fn base_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded;charset=UTF-8"),
    );
    headers.insert(
        reqwest::header::PRAGMA,
        HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
        ),
    );
    headers.insert(reqwest::header::ACCEPT, HeaderValue::from_static("*/*"));
    headers
}

/// Returns request headers, optionally inserting the currency/locale preference header.
///
/// # Errors
/// Returns an error if the formatted header string contains characters that
/// are not valid as an HTTP header value (in practice this never occurs since
/// all [`Currency`] codes and locale tags are plain ASCII).
fn get_headers(currency: Option<Currency>, language: &str, country: &str) -> Result<HeaderMap> {
    let mut headers = base_headers();
    if let Some(currency) = currency {
        let country_upper = country.to_uppercase();
        let currency_header = format!(
            r#"["{language}-{country_upper}","{country_upper}","{}",1,null,[-120],null,[[72534415,72446893,97456553,72399613]],1,[]]"#,
            currency
        );
        let header_value = reqwest::header::HeaderValue::from_str(&currency_header)
            .map_err(|e| anyhow::anyhow!("invalid currency header value: {e}"))?;
        headers.insert(
            reqwest::header::HeaderName::from_static("x-goog-ext-259736195-jspb"),
            header_value,
        );
    }
    Ok(headers)
}

/// Retrieves the frontend version from the Google Flights website.
async fn get_frontend_version() -> Option<String> {
    let client = Client::new();
    let headers = base_headers(); // no currency header needed for the version fetch
    let url = FLIGHTS_MAIN_PAGE.to_string();
    let res = client.get(&url).headers(headers).send().await.ok()?;

    let status = res.status();
    let final_url = res.url().to_string();
    // Only warn when the base path changes (different host or path).
    // Ignore minor redirects that only add/change query parameters.
    fn base_url(u: &str) -> &str {
        u.split_once('?').map_or(u, |(base, _)| base)
    }
    if base_url(&final_url) != base_url(&url) {
        tracing::warn!(
            original_url = %url,
            final_url = %final_url,
            status = %status,
            "main page request was redirected to a different URL"
        );
    } else {
        tracing::debug!(url = %final_url, status = %status, "main page response");
    }

    let response_body = res.text().await.ok()?;

    // Matches both:
    //   boq_travel-frontend-ui_20260527.01_p0  (old)
    //   boq_travel-frontend-flights-ui_20260527.01_p0  (new)
    let regex = match Regex::new(
        r"(boq_travel-frontend-[\w-]*ui_202[456789](01|02|03|04|05|06|07|08|09|10|11|12)\d{2}.\w{5,})",
    ) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "failed to compile version regex; using fallback version");
            return None;
        }
    };

    let result = regex
        .captures_iter(&response_body)
        .map(|f| f.extract::<2>())
        .next();

    match &result {
        Some((version, _)) => tracing::debug!(version, "frontend version extracted"),
        None => tracing::warn!(
            response_len = response_body.len(),
            "frontend version not found in main page response; using hardcoded fallback"
        ),
    }

    Some(result?.0.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal ApiClient without hitting the network (no frontend-version fetch).
    fn make_client() -> ApiClient {
        let quota = governor::Quota::per_second(NonZeroU32::new(100).unwrap());
        ApiClient {
            rate_limiter: Arc::new(DefaultDirectRateLimiter::direct(quota)),
            client: Arc::new(Client::new()),
            frontend_version: "test".into(),
            rate_limited: Arc::new(AtomicBool::new(false)),
            retry_config: RetryConfig::default(),
        }
    }

    #[test]
    fn not_rate_limited_by_default() {
        let client = make_client();
        assert!(!client.is_rate_limited());
    }

    #[test]
    fn rate_limited_flag_can_be_set_and_reset() {
        let client = make_client();
        client.rate_limited.store(true, Ordering::SeqCst);
        assert!(client.is_rate_limited());
        client.reset_rate_limit();
        assert!(!client.is_rate_limited());
    }

    #[test]
    fn clones_share_the_rate_limited_flag() {
        let client = make_client();
        let clone = client.clone();

        // Set on original — clone sees it.
        client.rate_limited.store(true, Ordering::SeqCst);
        assert!(clone.is_rate_limited());

        // Reset on clone — original sees it.
        clone.reset_rate_limit();
        assert!(!client.is_rate_limited());
    }

    #[test]
    fn rate_limited_error_is_downcasted() {
        let err = anyhow::Error::new(RateLimitedError);
        assert!(err.downcast_ref::<RateLimitedError>().is_some());
    }

    #[test]
    fn retry_config_default_values() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_attempts, 3);
        assert_eq!(cfg.base_delay_ms, 500);
        assert_eq!(cfg.cap_delay_ms, 30_000);
    }

    #[test]
    fn with_retry_config_overrides_defaults() {
        let client = make_client();
        let custom = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 200,
            cap_delay_ms: 10_000,
        };
        let client = client.with_retry_config(custom.clone());
        assert_eq!(client.retry_config.max_attempts, 5);
        assert_eq!(client.retry_config.base_delay_ms, 200);
        assert_eq!(client.retry_config.cap_delay_ms, 10_000);
    }

    #[test]
    fn retry_config_max_attempts_one_means_no_retries() {
        // max_attempts=1 → the loop runs exactly once; no retry occurs
        let cfg = RetryConfig {
            max_attempts: 1,
            base_delay_ms: 500,
            cap_delay_ms: 30_000,
        };
        // max(1,1) == 1, so range 0..1 has a single iteration
        assert_eq!(cfg.max_attempts.max(1), 1);
    }

    // -----------------------------------------------------------------------
    // Booking URL: meta-refresh extraction logic (offline, structural)
    // -----------------------------------------------------------------------

    /// The regex used in `resolve_booking_url` extracts the URL from a
    /// single-quoted meta-refresh response.
    #[test]
    fn booking_url_regex_extracts_single_quoted_url() {
        let html = r#"<meta content="0;url='https://example.com/book?foo=bar'" http-equiv="refresh">"#;
        let re = Regex::new(r#"(?i)url=['"]([^'"]+)['"]"#).unwrap();
        let extracted = re
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        assert_eq!(extracted, Some("https://example.com/book?foo=bar".to_string()));
    }

    /// The same regex handles double-quoted url values.
    #[test]
    fn booking_url_regex_extracts_double_quoted_url() {
        let _html = r#"<meta content="0;url=&quot;https://airline.com/booking?ref=123&amp;src=gf&quot;" http-equiv="refresh">"#;
        // double-quote form uses literal &quot; — after HTML-decode the url= value is double-quoted
        // here we test the raw pattern against a double-quoted variant directly
        let html2 = r#"<meta content='0;url="https://airline.com/booking?ref=123"' http-equiv="refresh">"#;
        let re = Regex::new(r#"(?i)url=['"]([^'"]+)['"]"#).unwrap();
        let extracted = re
            .captures(html2)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        assert_eq!(extracted, Some("https://airline.com/booking?ref=123".to_string()));
    }

    /// `&amp;` in the extracted URL is decoded to `&`.
    #[test]
    fn booking_url_amp_entity_is_decoded() {
        let raw = "https://example.com/book?foo=1&amp;bar=2";
        let decoded = raw.replace("&amp;", "&");
        assert_eq!(decoded, "https://example.com/book?foo=1&bar=2");
    }

    /// When the response contains no meta-refresh, the extraction returns `None`.
    #[test]
    fn booking_url_regex_returns_none_on_missing_url() {
        let html = "<html><head><title>Error</title></head><body>Not found</body></html>";
        let re = Regex::new(r#"(?i)url=['"]([^'"]+)['"]"#).unwrap();
        let extracted = re
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        assert_eq!(extracted, None);
    }
}
