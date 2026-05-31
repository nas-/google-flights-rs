use super::config::Currency;
use crate::parsers;
use crate::parsers::constants::{CLK_URL, FLIGHTS_MAIN_PAGE};
use crate::requests::config::Config;
use anyhow::Result;
use chrono::{Duration, Months, NaiveDate};
use governor::{DefaultDirectRateLimiter, Quota};
use parsers::calendar_graph_request::GraphRequestOptions;
use parsers::calendar_graph_response::GraphRawResponseContainer;
use parsers::city_request::CityRequestOptions;
use parsers::city_response::ResponseInnerBodyParsed;
use parsers::common::ToRequestBody;
use parsers::date_grid_request::{DateGridRequestOptions, DATE_GRID_MAX_CELLS};
use parsers::date_grid_response::{parse_date_grid_response, DateGridResponse};
use parsers::flight_request::FlightRequestOptions;
use parsers::flight_response::{create_raw_response_vec, FlightResponseContainer};
use parsers::offer_response::{self, OfferRawResponseContainer};
use regex::Regex;
use reqwest::header::HeaderMap;
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
}

impl ApiClient {
    /// Creates a new instance of `ApiClient` with a default rate limiter of 10 requests per second.
    pub async fn new() -> Self {
        let rate_limiter_quota = Quota::per_second(NonZeroU32::new(10).unwrap());
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
        }
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
        let options = CityRequestOptions::new(city, &self.frontend_version);
        let city_response: &str = &self.do_request(&options, None).await?.text().await?;
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
        let date_end_graph = &args.get_end_graph(months).to_string();
        let req_options = GraphRequestOptions::new(
            &args.departure,
            &args.destination,
            &args.departing_date,
            args.return_date.as_ref(),
            date_end_graph,
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
            .do_request(&req_options, Some(args.currency.clone()))
            .await?
            .text()
            .await?;
        let parsed = GraphRawResponseContainer::try_from(body.as_ref())?;
        Ok(parsed)
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
            .do_request(&req_options, Some(args.currency.clone()))
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
        let date_start = args.departing_date.to_string();
        let date_return: Option<String> = args.return_date.map(|f| f.to_string());
        tracing::info!("Requesting flights");
        let req_options = FlightRequestOptions::new(
            &args.departure,
            &args.destination,
            &date_start,
            date_return.as_deref(),
            args.travellers.clone(),
            &args.travel_class,
            &args.stop_options,
            &args.departing_times,
            &args.return_times,
            &args.stopover_max,
            &args.duration_max,
            &self.frontend_version,
            &args.fixed_flights,
        );

        let body = self
            .do_request(&req_options, Some(args.currency.clone()))
            .await?
            .text()
            .await?;
        let inner = create_raw_response_vec(body)?;
        Ok(inner)
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
        let date_start = args.departing_date.to_string();
        let date_return: Option<String> = args.return_date.map(|f| f.to_string());
        tracing::info!("Requesting offers");
        let req_options = FlightRequestOptions::new(
            &args.departure,
            &args.destination,
            &date_start,
            date_return.as_deref(),
            args.travellers.clone(),
            &args.travel_class,
            &args.stop_options,
            &args.departing_times,
            &args.return_times,
            &args.stopover_max,
            &args.duration_max,
            &self.frontend_version,
            &args.fixed_flights,
        );
        let body = self
            .do_request(&req_options, Some(args.currency.clone()))
            .await?
            .text()
            .await?;
        tracing::trace!(body = %body, "raw offer response body");
        let inner = offer_response::create_raw_response_offer_vec(body)?;
        Ok(inner)
    }

    /// Resolves a `click_token` from an [`OfferGroup`] or [`BookingSubOption`]
    /// into the final airline / OTA booking URL.
    ///
    /// Internally this POSTs the token to Google's click-tracker endpoint
    /// (`/travel/clk/f`) and extracts the redirect URL from the HTML
    /// `<meta http-equiv="refresh">` response.
    ///
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

        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let url = format!("{CLK_URL}?t={t}");
        // The token is URL-safe base64 so no extra percent-encoding is needed,
        // but we send it as a form body value.
        let body = format!("u={click_token}");

        tracing::debug!(%url, "resolving booking URL");

        let html = self
            .client
            .post(&url)
            .body(body)
            .headers(get_headers(None))
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

    /// Sends a single HTTP request, enforcing the shared rate-limit flag.
    ///
    /// Returns [`RateLimitedError`] (as an `anyhow::Error`) in two situations:
    /// - The flag is already set from a previous 429 on this client or any clone.
    /// - The server responds with HTTP 429 (the flag is then set for all clones).
    #[tracing::instrument(skip_all)]
    async fn do_request(
        &self,
        options: &impl ToRequestBody,
        currency: Option<Currency>,
    ) -> Result<Response> {
        // Refuse immediately if a previous request already received a 429.
        if self.rate_limited.load(Ordering::SeqCst) {
            return Err(anyhow::Error::new(RateLimitedError));
        }

        let req_payload = options.to_request_body()?;
        let headers = get_headers(currency);

        let decoded_body = percent_encoding::percent_decode_str(&req_payload.body)
            .decode_utf8_lossy()
            .into_owned();
        tracing::trace!(
            url = %req_payload.url,
            body = %decoded_body,
            ?headers,
            "Outgoing POST request"
        );

        let _permit = self
            .rate_limiter
            .until_n_ready(NonZeroU32::new(1).unwrap())
            .await;
        let res = self
            .client
            .post(req_payload.url)
            .body(req_payload.body)
            .headers(headers)
            .send()
            .await?;

        tracing::trace!(
            status = %res.status(),
            http_version = ?res.version(),
            "Response received"
        );

        match res.status() {
            StatusCode::OK => {}
            StatusCode::TOO_MANY_REQUESTS => {
                // Signal all clones to stop; they will return RateLimitedError
                // on their next attempt without hitting the network.
                self.rate_limited.store(true, Ordering::SeqCst);
                return Err(anyhow::Error::new(RateLimitedError));
            }
            status => tracing::warn!(
                http_version = ?res.version(),
                status_code = %status,
                "Unexpected HTTP response status"
            ),
        }

        Ok(res)
    }
}

/// Default headers for the requests.
/// Note that the header x-googl-batchexecute-bgr is not included as it is very hard to reverse the logic behind it.
/// This means that the the responses by the server are not always 100% accurate.
fn get_headers(currency: Option<Currency>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        "en-US,en;q=0.9".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        "application/x-www-form-urlencoded;charset=UTF-8"
            .parse()
            .unwrap(),
    );
    headers.insert(reqwest::header::PRAGMA, "no-cache".parse().unwrap());
    headers.insert(reqwest::header::CACHE_CONTROL, "no-cache".parse().unwrap());
    headers.insert(
        reqwest::header::USER_AGENT,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0"
            .parse()
            .unwrap(),
    );
    headers.insert(reqwest::header::ACCEPT, "*/*".parse().unwrap());

    if let Some(currency) = currency {
        let currency_header = format!(
            r#"["en-GB","GB","{}",1,null,[-120],null,[[72534415,72446893,97456553,72399613]],1,[]]"#,
            currency
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-goog-ext-259736195-jspb"),
            reqwest::header::HeaderValue::from_str(&currency_header).unwrap(),
        );
    }
    headers
}


/// Retrieves the frontend version from the Google Flights website.
async fn get_frontend_version() -> Option<String> {
    let client = Client::new();
    let headers = get_headers(None);
    let url = FLIGHTS_MAIN_PAGE.to_string();
    let res = client.get(&url).headers(headers).send().await.ok()?;

    let status = res.status();
    let final_url = res.url().to_string();
    if final_url != url {
        tracing::warn!(
            original_url = %url,
            final_url = %final_url,
            status = %status,
            "main page request was redirected"
        );
    } else {
        tracing::debug!(url = %final_url, status = %status, "main page response");
    }

    let response_body = res.text().await.ok()?;

    // Matches both:
    //   boq_travel-frontend-ui_20260527.01_p0  (old)
    //   boq_travel-frontend-flights-ui_20260527.01_p0  (new)
    let regex = Regex::new(
        r"(boq_travel-frontend-[\w-]*ui_202[456789](01|02|03|04|05|06|07|08|09|10|11|12)\d{2}.\w{5,})",
    )
    .unwrap();

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
}


