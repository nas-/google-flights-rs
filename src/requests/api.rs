use crate::requests::config::Config;
use governor::{DefaultDirectRateLimiter, Quota};
use parsers::calendar_graph_request::GraphRequestOptions;
use parsers::calendar_graph_response::GraphRawResponseContainer;
use parsers::city_request::CityRequestOptions;
use parsers::city_response::ResponseInnerBodyParsed;
use parsers::common::{FixedFlights, ToRequestBody};
use parsers::flight_request::FlightRequestOptions;
use parsers::flight_response::{create_raw_response_vec, RawResponse};
use parsers::offer_response::{self, OfferRawResponse};
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiClient {
    pub rate_limiter: Arc<DefaultDirectRateLimiter>,
    pub client: Arc<Client>,
    frontend_version: String,
}

impl ApiClient {
    pub async fn new(rate_limiter_quota: Quota) -> Self {
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

    pub async fn request_city(&self, city: &str) -> anyhow::Result<ResponseInnerBodyParsed> {
        let options = CityRequestOptions::new(city, &self.frontend_version);
        let city_response: &str = &self.do_request(&options).await?.text().await?;
        let cities_res = ResponseInnerBodyParsed::try_from(city_response)?;
        Ok(cities_res)
    }

    pub async fn request_graph(&self, args: &Config) -> anyhow::Result<GraphRawResponseContainer> {
        let req_options = GraphRequestOptions::new(
            &args.departure,
            &args.destination,
            &args.departing_date,
            args.return_date.as_ref(),
            &args.date_end_graph,
            args.travellers.clone(),
            &args.travel_class,
            &args.stop_options,
            &args.departing_times,
            &args.return_times,
            &args.stopover_max,
            &args.duration_max,
            &self.frontend_version,
        );

        let body = self.do_request(&req_options).await?.text().await?;
        let parsed = GraphRawResponseContainer::try_from(body.as_ref())?;
        Ok(parsed)
    }

    pub async fn request_flights(
        &self,
        args: &Config,
        fixed_flights: &FixedFlights,
    ) -> anyhow::Result<Vec<RawResponse>> {
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
            fixed_flights,
        );

        let body = self.do_request(&req_options).await?.text().await?;
        let inner = create_raw_response_vec(body)?;
        Ok(inner)
    }

    pub async fn request_offer(
        &self,
        args: &Config,
        fixed_flights: &FixedFlights,
    ) -> anyhow::Result<Vec<OfferRawResponse>> {
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
            fixed_flights,
        );
        let body = self.do_request(&req_options).await?.text().await?;
        let inner = offer_response::create_raw_response_offer_vec(body)?;
        Ok(inner)
    }

    async fn do_request(&self, options: &impl ToRequestBody) -> Result<Response, reqwest::Error> {
        let req_payload = options.to_request_body();
        let headers = get_headers();

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

fn get_headers() -> HeaderMap {
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
    headers
}

async fn get_frontend_version() -> Option<String> {
    let client = Client::new();
    let headers = get_headers();
    let url = "https://www.google.com/travel/flights".to_string();
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
