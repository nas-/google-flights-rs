//! # gflights — unofficial Google Flights client for Rust
//!
//! An async library that talks to the same endpoints used by the Google Flights
//! website.  It handles request encoding, response parsing, rate limiting, and
//! automatic retry for transient server errors.
//!
//! ## Data flow
//!
//! ```text
//! ConfigBuilder::build()  →  Config
//!      ↓
//! ApiClient::request_flights(&config)  →  FlightResponseContainer
//!      │                                       │
//!      │                               Vec<RawResponse>
//!      │                                    │
//!      │                          ItineraryContainerList
//!      │                              │              │
//!      │                       best_flights    other_flights
//!      │                              └──────┬──────┘
//!      │                              Vec<ItineraryContainer>
//!      │                                       │
//!      │                               ┌───────┴──────────┐
//!      │                          Itinerary         ItineraryCost
//!      │                        (flight details,    (price, token)
//!      │                         duration, CO2)
//!      │
//! ApiClient::request_offer(&config)   →  OfferRawResponseContainer
//!      │                                       │
//!      │                               Vec<OfferGroup>
//!      │                              (price, airline, click_token)
//!      │
//! ApiClient::resolve_booking_url(token)  →  String (booking URL)
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use gflights::requests::{api::ApiClient, config::Config};
//! use chrono::{Duration, Utc};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = ApiClient::new().await;
//!     let today = Utc::now().date_naive();
//!
//!     let config = Config::builder()
//!         .departure("LHR", &client).await?
//!         .destination("JFK", &client).await?
//!         .departing_date(today + Duration::days(14))
//!         .build()?;
//!
//!     let results = client.request_flights(&config).await?;
//!     for resp in &results.responses {
//!         if let Some(flights) = resp.maybe_get_all_flights() {
//!             for f in &flights {
//!                 println!("{} — {} min — {:?}",
//!                     f.itinerary.flight_by,
//!                     f.itinerary.total_time_minutes,
//!                     f.itinerary_cost.trip_cost);
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```

pub mod parsers;
pub mod protos;
pub mod requests;

/// Result type from [`requests::api::ApiClient::cheapest_dates`].
pub use parsers::response::date_grid_response::CheapDate;
/// Re-exported for downcasting: `err.downcast_ref::<RateLimitedError>()`.
pub use requests::api::RateLimitedError;
/// Re-exported for configuring retry behaviour on [`requests::api::ApiClient`].
pub use requests::api::RetryConfig;
