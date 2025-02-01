use super::config::Currency;
use crate::parsers;
use crate::parsers::constants::FLIGHTS_MAIN_PAGE;
use crate::requests::config::Config;
use anyhow::Result;
use chrono::Months;
use governor::{DefaultDirectRateLimiter, Quota};
use parsers::calendar_graph_request::GraphRequestOptions;
use parsers::calendar_graph_response::GraphRawResponseContainer;
use parsers::city_request::CityRequestOptions;
use parsers::city_response::ResponseInnerBodyParsed;
use parsers::common::ToRequestBody;
use parsers::flight_request::FlightRequestOptions;
use parsers::flight_response::{create_raw_response_vec, FlightResponseContainer};
use parsers::offer_response::{self, OfferRawResponseContainer};
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};
use std::num::NonZeroU32;
use std::sync::Arc;

/// The `ApiClient` struct is used to send requests to the Google Flights website.
#[derive(Clone)]
pub struct ApiClient {
    pub rate_limiter: Arc<DefaultDirectRateLimiter>,
    pub client: Arc<Client>,
    frontend_version: String,
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
                .unwrap_or("boq_travel-frontend-ui_20240110.02_p0".into()),
        }
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

    /// Sends a request to retrieve flight data.
    ///
    /// # Arguments
    ///
    /// * `args` - The configuration options for the request.
    ///
    /// # Returns
    ///
    /// Returns a `FlightResponseContainer` object containing the parsed response.
    pub async fn request_flights(&self, args: &Config) -> Result<FlightResponseContainer> {
        let date_start = args.departing_date.to_string();
        let date_return: Option<String> = args.return_date.map(|f| f.to_string());
        println!(
            "Processing {:?} flights from {:?} to {:?} date {} in class {:?}",
            args.stop_options,
            args.departure.location_name,
            args.destination.location_name,
            date_start,
            args.travel_class
        );
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
    pub async fn request_offer(&self, args: &Config) -> Result<OfferRawResponseContainer> {
        let date_start = args.departing_date.to_string();
        let date_return: Option<String> = args.return_date.map(|f| f.to_string());
        println!(
            "Processing {:?} flights from {:?} to {:?} date {} in class {:?}",
            args.stop_options,
            args.departure.location_name,
            args.destination.location_name,
            date_start,
            args.travel_class
        );
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
        let inner = offer_response::create_raw_response_offer_vec(body)?;
        Ok(inner)
    }

    /// Sends a request to retrieve flight data.
    async fn do_request(
        &self,
        options: &impl ToRequestBody,
        currency: Option<Currency>,
    ) -> Result<Response> {
        let req_payload = options.to_request_body()?;
        let headers = get_headers(currency);

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
        match res.status() {
            StatusCode::OK => (),
            _ => eprintln!("Response: {:?} {}", res.version(), res.status()),
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
    let res = client.get(url).headers(headers).send().await.ok()?;

    let response_body = res.text().await.ok()?;
    let regex = Regex::new(
        r"(boq_travel-frontend-ui_202[456](01|02|03|04|05|06|07|08|09|10|11|12)\d{2}.\w{5,})",
    )
    .unwrap();

    let result = regex
        .captures_iter(&response_body)
        .map(|f| f.extract::<2>())
        .next()?;

    let a = result.0.to_string();

    Some(a)
}
