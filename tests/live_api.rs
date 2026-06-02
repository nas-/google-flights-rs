//! Integration tests that call live Google Flights servers.
//!
//! These tests are marked `#[ignore]` so they are skipped by default, and
//! additionally gated behind the `RUN_LIVE_TESTS` environment variable so
//! they are never accidentally executed on CI servers (which set neither
//! that variable nor the explicit opt-in flag).
//!
//! # Running locally
//!
//! ```sh
//! # Run all live tests:
//! RUN_LIVE_TESTS=1 cargo test --test live_api -- --include-ignored
//!
//! # Run a single live test:
//! RUN_LIVE_TESTS=1 cargo test --test live_api oneway_search_returns_flights -- --include-ignored
//! ```
//!
//! # CI behaviour
//!
//! CI pipelines typically run `cargo test` or `cargo test -- --include-ignored`.
//! Neither invocation executes these tests because:
//!
//! 1. `#[ignore]` keeps them out of the default run.
//! 2. Even if `--include-ignored` is passed, the `require_live!()` guard at
//!    the start of every test body returns `Ok(())` immediately unless the
//!    `RUN_LIVE_TESTS` environment variable is set to a non-empty value.
//!
//! # What these tests do NOT assert
//!
//! - Exact prices (dynamic)
//! - Exact flight counts (seasonal, availability)
//! - Specific airlines operating a route
//! - Exact number of "cheaper date" suggestions
//!
//! # What these tests DO assert
//!
//! - The API returns parseable, structurally valid responses
//! - Airport codes are 3-character uppercase ASCII
//! - Airline codes are non-empty
//! - Departure tokens are non-empty (needed for follow-up requests)
//! - Popular routes always return at least one flight
//! - Generated itinerary URLs have the expected base URL

use anyhow::Result;
use chrono::{Duration, Months, Utc};
use gflights::parsers::common::{AirlineCode, AirlineFilter, Alliance};
use gflights::requests::{api::ApiClient, config::Config};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Shared client (avoids spinning up a fresh rate-limiter for every test)
// ---------------------------------------------------------------------------

/// A process-wide live client initialised at most once.
///
/// `tokio::sync::OnceCell` is safe to share across the independent runtimes
/// that `#[tokio::test]` creates because, once the value is present, every
/// subsequent `get_or_init` call resolves immediately without touching the
/// runtime-internal waker machinery.
static LIVE_CLIENT: tokio::sync::OnceCell<ApiClient> = tokio::sync::OnceCell::const_new();

async fn shared_client() -> &'static ApiClient {
    LIVE_CLIENT
        .get_or_init(|| async { ApiClient::new().await })
        .await
}

/// Early-returns `Ok(())` from the enclosing async fn unless `RUN_LIVE_TESTS`
/// is set to a non-empty value in the environment.
///
/// This is the second line of defence against live tests running on CI:
/// the first is `#[ignore]`, but some CI pipelines pass `--include-ignored`.
macro_rules! require_live {
    () => {
        match std::env::var("RUN_LIVE_TESTS") {
            Ok(v) if !v.is_empty() => {}
            _ => {
                eprintln!("[live_api] skipping — set RUN_LIVE_TESTS=1 to run live tests");
                return Ok(());
            }
        }
    };
}

/// Returns a `NaiveDate` that is `n` days from today (UTC).
fn days_from_now(n: i64) -> chrono::NaiveDate {
    (Utc::now() + Duration::days(n)).date_naive()
}

/// Asserts that `code` looks like a valid IATA airport code (3 uppercase ASCII letters).
fn assert_airport_code(code: &str, label: &str) {
    assert_eq!(
        code.len(),
        3,
        "{label}: expected 3-char airport code, got {code:?}"
    );
    assert!(
        code.chars().all(|c| c.is_ascii_uppercase()),
        "{label}: airport code should be uppercase ASCII, got {code:?}"
    );
}

// ---------------------------------------------------------------------------
// City / location lookup
// ---------------------------------------------------------------------------

/// A city lookup by full English name returns a non-empty identifier and name.
#[tokio::test]
#[ignore = "requires live network"]
async fn city_lookup_by_full_name() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let result = client.request_city("London").await?;
    let loc = result.to_city_list();

    assert!(
        !loc.loc_identifier.is_empty(),
        "loc_identifier should not be empty"
    );
    assert!(
        loc.location_name
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "location_name should be set and non-empty"
    );

    Ok(())
}

/// Multiple well-known cities all resolve without error.
#[tokio::test]
#[ignore = "requires live network"]
async fn city_lookup_several_cities() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    for city in ["Madrid", "Paris", "Tokyo", "New York"] {
        let loc = client.request_city(city).await?.to_city_list();
        assert!(
            !loc.loc_identifier.is_empty(),
            "lookup for {city:?} returned an empty identifier"
        );
        assert!(
            loc.location_name.is_some(),
            "lookup for {city:?} returned no location_name"
        );
    }

    Ok(())
}

/// When an IATA code is used directly, the config builder skips the city API
/// and sets location_name to the code itself (so logs show the code, not None).
#[tokio::test]
#[ignore = "requires live network"]
async fn iata_code_sets_location_name_to_code() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    assert_eq!(config.departure[0].loc_identifier, "LHR");
    assert_eq!(config.destination[0].loc_identifier, "JFK");
    assert_eq!(
        config.departure[0].location_name.as_deref(),
        Some("LHR"),
        "IATA departure should use code as location_name"
    );
    assert_eq!(
        config.destination[0].location_name.as_deref(),
        Some("JFK"),
        "IATA destination should use code as location_name"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// One-way flight search
// ---------------------------------------------------------------------------

/// A one-way search on a very busy route (LHR→JFK) returns at least one flight.
#[tokio::test]
#[ignore = "requires live network"]
async fn oneway_search_returns_flights() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    let response = client.request_flights(&config).await?;

    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(!flights.is_empty(), "expected ≥1 flight for LHR→JFK");

    Ok(())
}

/// Every flight returned has structurally valid fields.
///
/// Specifically:
/// - departure / destination airport codes are 3-char uppercase
/// - airline code (`flight_by`) is non-empty
/// - `departure_token` is non-empty (needed by follow-up requests)
/// - at least one leg exists in every itinerary
#[tokio::test]
#[ignore = "requires live network"]
async fn flight_results_have_valid_structure() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    let response = client.request_flights(&config).await?;

    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(
        !flights.is_empty(),
        "need at least one flight to validate structure"
    );

    for (i, itinerary_container) in flights.iter().enumerate() {
        let itin = &itinerary_container.itinerary;

        assert!(
            !itin.flight_by.is_empty(),
            "flight[{i}]: flight_by (airline code) should not be empty"
        );
        assert!(
            !itin.flight_details.is_empty(),
            "flight[{i}]: should have at least one leg"
        );

        for (j, leg) in itin.flight_details.iter().enumerate() {
            assert_airport_code(
                &leg.departure_airport_code,
                &format!("flight[{i}] leg[{j}] dep"),
            );
            assert_airport_code(
                &leg.destination_airport_code,
                &format!("flight[{i}] leg[{j}] arr"),
            );
            assert!(
                !leg.airplane_info.code.is_empty(),
                "flight[{i}] leg[{j}]: airplane_info.code should not be empty"
            );
        }

        assert!(
            !itinerary_container
                .itinerary_cost
                .departure_token
                .is_empty(),
            "flight[{i}]: departure_token must be non-empty (needed for follow-up requests)"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Return-trip two-leg flow
// ---------------------------------------------------------------------------

/// Full return-trip flow:
///   1. Search outbound  → select first result
///   2. Search return    → select first result
///   3. Generate URL     → must start with google.com/travel/flights
///
/// This mirrors the happy path the `flights` example uses.
#[tokio::test]
#[ignore = "requires live network"]
async fn return_flight_two_leg_flow_produces_valid_url() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .return_date(days_from_now(21))
        .build()?;

    // --- Outbound leg ---
    let out_resp = client.request_flights(&config).await?;
    let first_flight = out_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no outbound flights found for LHR→JFK");

    assert!(!first_flight.itinerary.flight_details.is_empty());
    assert!(!first_flight.itinerary_cost.departure_token.is_empty());

    config
        .fixed_flights
        .add_element(first_flight)
        .expect("failed to add first leg");

    // --- Return leg ---
    let ret_resp = client.request_flights(&config).await?;
    let return_flight = ret_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no return flights found for JFK→LHR");

    assert!(!return_flight.itinerary.flight_details.is_empty());

    // Last leg of the return should land at LHR or a London airport
    let last_leg = return_flight.itinerary.flight_details.last().unwrap();
    assert_airport_code(
        &last_leg.destination_airport_code,
        "return last leg destination",
    );

    config
        .fixed_flights
        .add_element(return_flight)
        .expect("failed to add second leg");

    // --- URL ---
    let url = config.to_flight_url();
    assert!(
        url.starts_with("https://www.google.com/travel/flights"),
        "itinerary URL should start with google flights base: {url}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Price graph
// ---------------------------------------------------------------------------

/// The price graph for a popular route over 2 months returns at least one
/// date-price suggestion, and every suggestion has a valid departure date
/// (in the future relative to now).
#[tokio::test]
#[ignore = "requires live network"]
async fn price_graph_returns_future_dates() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    let graph = client.request_graph(&config, Months::new(2)).await?;
    let suggestions = graph.get_all_graphs();

    assert!(
        !suggestions.is_empty(),
        "expected at least one price-graph data point for LHR→JFK over 2 months"
    );

    for (i, s) in suggestions.iter().enumerate() {
        assert!(
            s.proposed_departure_date >= today,
            "suggestion[{i}]: proposed_departure_date {:?} should not be in the past",
            s.proposed_departure_date
        );
        // If a trip cost is present it should have a positive price
        if let Some(ref cost) = s.proposed_trip_cost {
            assert!(
                cost.trip_cost.price > 0,
                "suggestion[{i}]: price should be positive"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Cheaper-travel suggestions (best-price nearby dates)
// ---------------------------------------------------------------------------

/// When the API returns cheaper-travel-on-different-dates suggestions, every
/// suggestion's departure date is parseable and non-None.
/// This test does not assert whether suggestions exist — their presence is
/// route- and season-dependent — but if they exist the data must be valid.
#[tokio::test]
#[ignore = "requires live network"]
async fn cheaper_dates_suggestions_are_structurally_valid() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(30))
        .build()?;

    let response = client.request_flights(&config).await?;

    for raw in &response.responses {
        let containers = match raw.travel_cheaper_different_date.as_ref() {
            Some(v) => v,
            None => continue,
        };
        for container in containers {
            if let Some(ref s) = container.different_dates {
                assert!(
                    s.proposed_departure_date >= today,
                    "cheaper-date suggestion has a past departure date: {:?}",
                    s.proposed_departure_date
                );
                if let Some(ref cost) = s.proposed_trip_cost {
                    assert!(
                        cost.trip_cost.price > 0,
                        "cheaper-date suggestion price should be positive"
                    );
                }
            }
            if let Some(ref places) = container.different_airport_or_dates {
                for s in places.dates.iter().flatten() {
                    assert!(
                        s.proposed_departure_date >= today,
                        "different-airport suggestion has a past departure date: {:?}",
                        s.proposed_departure_date
                    );
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Usual price bound
// ---------------------------------------------------------------------------

/// `get_usual_price_bound()` either returns None (not available for this query)
/// or a positive integer.
#[tokio::test]
#[ignore = "requires live network"]
async fn usual_price_bound_is_positive_when_present() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(30))
        .build()?;

    let response = client.request_flights(&config).await?;

    if let Some(bound) = response.get_usual_price_bound() {
        assert!(
            bound > 0,
            "usual_price_bound should be a positive integer, got {bound}"
        );
    }
    // None is also acceptable — not all routes/dates include this

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-airport search
// ---------------------------------------------------------------------------

/// Searching from two London airports (LHR + LGW) to JFK returns at least one
/// flight.  This exercises the multi-airport path end-to-end against the live
/// API: both airport codes must appear in the serialised request body.
#[tokio::test]
#[ignore = "requires live network"]
async fn multi_airport_departure_returns_flights() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .add_departure("LGW", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    assert_eq!(
        config.departure.len(),
        2,
        "should have LHR and LGW as departure airports"
    );

    let response = client.request_flights(&config).await?;
    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(!flights.is_empty(), "expected ≥1 flight for (LHR|LGW)→JFK");

    Ok(())
}

/// Searching to two New York airports (JFK + EWR) from LHR returns at least
/// one flight, verifying multi-airport destination support.
#[tokio::test]
#[ignore = "requires live network"]
async fn multi_airport_destination_returns_flights() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .add_destination("EWR", &client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    assert_eq!(
        config.destination.len(),
        2,
        "should have JFK and EWR as destination airports"
    );

    let response = client.request_flights(&config).await?;
    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(!flights.is_empty(), "expected ≥1 flight for LHR→(JFK|EWR)");

    Ok(())
}

// ---------------------------------------------------------------------------
// Offer / booking options
// ---------------------------------------------------------------------------

/// Full offer flow for a return trip LHR → JFK:
///   1. Search outbound  → select first result
///   2. Search return    → select first result
///   3. Request offers   → at least one offer with a sensible price
///
/// We only check that the price is within a very wide but sanity-checking
/// range (> 0 and < 20 000 EUR/USD) — exact prices change daily.
#[tokio::test]
#[ignore = "requires live network"]
async fn offer_request_returns_prices_for_lhr_jfk() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(30))
        .return_date(days_from_now(37))
        .build()?;

    // --- Outbound leg ---
    let out_resp = client.request_flights(&config).await?;
    let first_out = out_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no outbound flights for LHR→JFK");

    config
        .fixed_flights
        .add_element(first_out)
        .expect("failed to fix outbound leg");

    // --- Return leg ---
    let ret_resp = client.request_flights(&config).await?;
    let first_ret = ret_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no return flights for JFK→LHR");

    config
        .fixed_flights
        .add_element(first_ret)
        .expect("failed to fix return leg");

    // --- Offers ---
    let offer_container = client.request_offer(&config).await?;

    // Flatten all offer groups across all inner responses
    let all_prices: Vec<(Vec<String>, i32)> = offer_container
        .response
        .iter()
        .filter_map(|r| r.get_offer_prices())
        .flatten()
        .collect();

    assert!(
        !all_prices.is_empty(),
        "expected at least one booking offer for LHR→JFK round trip, got none"
    );

    for (airlines, price) in &all_prices {
        assert!(
            *price > 0,
            "offer price should be positive, got {price} (airlines: {airlines:?})"
        );
        assert!(
            *price < 20_000,
            "offer price looks unreasonably high: {price} (airlines: {airlines:?})"
        );
        assert!(
            !airlines.is_empty(),
            "offer should have at least one airline name, got empty list (price: {price})"
        );
    }

    Ok(())
}

/// Offers have at least one price in a plausible transatlantic range (> 200).
///
/// This is a weaker sanity-check companion to `offer_request_returns_prices_for_lhr_jfk`:
/// we assert that Google returns at least one offer that is neither suspiciously
/// cheap nor obviously a data-parse artifact.
#[tokio::test]
#[ignore = "requires live network"]
async fn offer_prices_are_in_plausible_range_for_lhr_jfk() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(30))
        .return_date(days_from_now(37))
        .build()?;

    // Select outbound
    let out_resp = client.request_flights(&config).await?;
    let first_out = out_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no outbound flights for LHR→JFK");
    config.fixed_flights.add_element(first_out).unwrap();

    // Select return
    let ret_resp = client.request_flights(&config).await?;
    let first_ret = ret_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no return flights for JFK→LHR");
    config.fixed_flights.add_element(first_ret).unwrap();

    // Offers
    let offer_container = client.request_offer(&config).await?;
    let all_prices: Vec<i32> = offer_container
        .response
        .iter()
        .filter_map(|r| r.get_offer_prices())
        .flatten()
        .map(|(_, p)| p)
        .collect();

    assert!(
        !all_prices.is_empty(),
        "expected at least one offer for LHR→JFK round trip"
    );

    // At least one offer should be over 200 (transatlantic round trip is never free)
    let has_plausible = all_prices.iter().any(|&p| p > 200);
    assert!(
        has_plausible,
        "expected at least one offer > 200, got: {:?}",
        all_prices
    );

    Ok(())
}

/// Offer sub-options (per-OTA booking channels) are structurally valid when present.
///
/// When Google returns per-channel prices they must all be positive and
/// each channel must have at least one partner name.
#[tokio::test]
#[ignore = "requires live network"]
async fn offer_sub_options_are_structurally_valid_for_lhr_jfk() -> Result<()> {
    require_live!();
    let client = ApiClient::new().await;

    let config = Config::builder()
        .departure("LHR", &client)
        .await?
        .destination("JFK", &client)
        .await?
        .departing_date(days_from_now(30))
        .return_date(days_from_now(37))
        .build()?;

    // Select outbound
    let out_resp = client.request_flights(&config).await?;
    let first_out = out_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no outbound flights for LHR→JFK");
    config.fixed_flights.add_element(first_out).unwrap();

    // Select return
    let ret_resp = client.request_flights(&config).await?;
    let first_ret = ret_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no return flights for JFK→LHR");
    config.fixed_flights.add_element(first_ret).unwrap();

    // Offers
    let offer_container = client.request_offer(&config).await?;

    for (i, raw_resp) in offer_container.response.iter().enumerate() {
        for (j, group) in raw_resp.offers.iter().enumerate() {
            for (k, sub) in group.sub_options.iter().enumerate() {
                if let Some(price) = sub.price {
                    assert!(
                        price > 0,
                        "response[{i}] group[{j}] sub_option[{k}]: price should be positive, got {price}"
                    );
                }
                if sub.price.is_some() {
                    assert!(
                        !sub.partner_names.is_empty(),
                        "response[{i}] group[{j}] sub_option[{k}]: has a price but no partner names"
                    );
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Locale parameters
// ---------------------------------------------------------------------------

/// A search with French locale (language="fr", country="FR") returns a
/// structurally valid response — verifies non-English locale is threaded
/// through to the API endpoint without parse errors.
#[tokio::test]
#[ignore = "requires live network"]
async fn search_with_french_locale_parses_ok() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    let config = Config::builder()
        .departure("CDG", client)
        .await?
        .destination("JFK", client)
        .await?
        .departing_date(days_from_now(14))
        .language("fr".to_string())
        .country("FR".to_string())
        .build()?;

    let response = client.request_flights(&config).await?;

    // Must parse without error and return at least one flight on a busy route.
    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(
        !flights.is_empty(),
        "expected ≥1 flight for CDG→JFK with fr-FR locale"
    );

    // Structural sanity on the first result.
    let first = &flights[0].itinerary;
    assert!(!first.flight_by.is_empty(), "flight_by should not be empty");
    assert!(
        !first.flight_details.is_empty(),
        "flight_details should not be empty"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------

/// Three concurrent tasks sharing one `ApiClient` all return results without
/// panicking. Exercises the rate-limiter and internal state under parallelism.
#[tokio::test]
#[ignore = "requires live network"]
async fn concurrent_requests_all_succeed() -> Result<()> {
    require_live!();
    // Use a freshly constructed client wrapped in Arc so all three tasks share
    // the same rate-limiter bucket.
    let client = Arc::new(ApiClient::new().await);

    let mut handles = Vec::new();
    for _ in 0..3 {
        let c = Arc::clone(&client);
        handles.push(tokio::spawn(async move {
            let config = Config::builder()
                .departure("LHR", &c)
                .await?
                .destination("JFK", &c)
                .await?
                .departing_date(days_from_now(14))
                .build()?;
            let resp = c.request_flights(&config).await?;
            let count = resp
                .responses
                .iter()
                .filter_map(|r| r.maybe_get_all_flights())
                .flatten()
                .count();
            anyhow::Ok(count)
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let count = handle.await??;
        assert!(
            count > 0,
            "concurrent task {i} returned 0 flights — expected ≥1 for LHR→JFK"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Booking-URL / click tokens
// ---------------------------------------------------------------------------

/// Every offer that carries a price also carries a non-empty click token.
/// The click token is the opaque string used by `resolve_booking_url()` to
/// obtain the final airline/OTA redirect URL.
#[tokio::test]
#[ignore = "requires live network"]
async fn offer_click_tokens_are_nonempty() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    let config = Config::builder()
        .departure("LHR", client)
        .await?
        .destination("JFK", client)
        .await?
        .departing_date(days_from_now(30))
        .return_date(days_from_now(37))
        .build()?;

    // Fix outbound leg.
    let out_resp = client.request_flights(&config).await?;
    let first_out = out_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no outbound flights for LHR→JFK");
    config.fixed_flights.add_element(first_out)?;

    // Fix return leg.
    let ret_resp = client.request_flights(&config).await?;
    let first_ret = ret_resp
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .expect("no return flights for JFK→LHR");
    config.fixed_flights.add_element(first_ret)?;

    let offers = client.request_offer(&config).await?;

    // Collect offers that have a price — those must also carry a click token.
    let priced_offers: Vec<_> = offers
        .response
        .iter()
        .flat_map(|r| &r.offers)
        .filter(|o| o.price.is_some())
        .collect();

    assert!(
        !priced_offers.is_empty(),
        "expected ≥1 priced offer for LHR→JFK round trip"
    );

    for (i, offer) in priced_offers.iter().enumerate() {
        let token = offer
            .click_token
            .as_deref()
            .unwrap_or_else(|| panic!("offer[{i}] has a price but no click_token"));
        assert!(
            !token.is_empty(),
            "offer[{i}] click_token must be non-empty"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Negative / error inputs
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Airline + Alliance mixed filter
// ---------------------------------------------------------------------------

/// Verifies that the Google API accepts a mixed include filter containing both
/// an IATA airline code (`BA`) **and** an alliance name (`ONEWORLD`).
///
/// Serialisation produces `["BA","ONEWORLD"]` — the question under test is
/// whether Google actually processes this at runtime (unit tests only cover
/// the serialisation side).  We assert:
/// - No parse or HTTP error from the API.
/// - The response is non-empty (BA and other Oneworld carriers fly LHR→JFK).
#[tokio::test]
#[ignore = "requires live network"]
async fn live_mixed_airline_alliance_include_filter() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    let mut config = Config::builder()
        .departure("LHR", client)
        .await?
        .destination("JFK", client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    // Mixed include: BA (IATA code) + OneWorld (alliance) in one array.
    config.airlines_include = vec![
        AirlineFilter::Airline(AirlineCode::new("BA")?),
        AirlineFilter::Alliance(Alliance::OneWorld),
    ];

    let response = client.request_flights(&config).await?;

    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    // The API must accept the mixed array without returning a parse error.
    // LHR→JFK is served by OneWorld members so we expect real results.
    assert!(
        !flights.is_empty(),
        "expected ≥1 flight with mixed BA+OneWorld include filter on LHR→JFK"
    );

    Ok(())
}

/// Verifies that the Google API accepts a mixed exclude filter containing both
/// an IATA airline code (`FR` / Ryanair) **and** an alliance (`SKYTEAM`).
///
/// Serialisation produces `["FR","SKYTEAM"]`.  We assert:
/// - No parse or HTTP error from the API.
/// - No returned flight has `flight_by == "FR"` (Ryanair excluded).
/// - No returned flight is operated by a common SkyTeam carrier
///   (AF, KL, DL) — checked as a best-effort signal on a route where
///   those airlines typically appear.
#[tokio::test]
#[ignore = "requires live network"]
async fn live_mixed_airline_alliance_exclude_filter() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    let mut config = Config::builder()
        .departure("CDG", client)
        .await?
        .destination("JFK", client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    // Mixed exclude: FR (Ryanair, IATA code) + SkyTeam (alliance).
    config.airlines_exclude = vec![
        AirlineFilter::Airline(AirlineCode::new("FR")?),
        AirlineFilter::Alliance(Alliance::SkyTeam),
    ];

    let response = client.request_flights(&config).await?;

    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    // Even with exclusions CDG→JFK has non-SkyTeam options (e.g. AA, BA, LH).
    assert!(
        !flights.is_empty(),
        "expected ≥1 non-SkyTeam/non-FR flight for CDG→JFK"
    );

    // Ryanair should not appear (it doesn't fly transatlantic anyway, but the
    // filter must not break the response).
    for (i, f) in flights.iter().enumerate() {
        assert_ne!(
            f.itinerary.flight_by, "FR",
            "flight[{i}]: Ryanair (FR) should have been excluded"
        );
        // Best-effort: the most prominent SkyTeam transatlantic carriers.
        // Google may suppress results entirely rather than enumerate, so only
        // assert when the carrier is directly named.
        for code in &["AF", "KL", "DL"] {
            assert_ne!(
                &f.itinerary.flight_by, code,
                "flight[{i}]: SkyTeam carrier {code} should have been excluded"
            );
        }
    }

    Ok(())
}

/// Searching with the fictional IATA code "XXX" must not panic.
///
/// The API may:
///   (a) refuse to build the config (`build()` returns `Err`), or
///   (b) send the request and return an empty result list, or
///   (c) interpret "XXX" as a city lookup and return results for another airport.
///
/// All outcomes are acceptable — what is NOT acceptable is an unhandled panic
/// or an error that propagates without being caught by the error types.
#[tokio::test]
#[ignore = "requires live network"]
async fn invalid_iata_xxx_does_not_panic() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    // City lookup for "XXX" — may succeed (returning some location) or fail.
    let build_result = Config::builder()
        .departure("XXX", client)
        .await?
        .destination("JFK", client)
        .await?
        .departing_date(days_from_now(14))
        .build();

    let config = match build_result {
        Err(_) => {
            // Config build rejected the input — valid outcome.
            return Ok(());
        }
        Ok(c) => c,
    };

    // If config built, the flight search must either parse cleanly or return
    // a typed error — no panics allowed.
    match client.request_flights(&config).await {
        Err(_) => {
            // Typed error propagated correctly — valid outcome.
        }
        Ok(resp) => {
            // Parse the response regardless of flight count — must not panic.
            let _count = resp
                .responses
                .iter()
                .filter_map(|r| r.maybe_get_all_flights())
                .flatten()
                .count();
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Rail station / non-airport IATA codes
// ---------------------------------------------------------------------------

/// Rail station codes (Amadeus X-prefix convention, e.g. XRJ = Rome Termini,
/// XVQ = Venice Santa Lucia) must not cause panics or unrecoverable errors.
///
/// Google Flights does not know about rail codes so results will typically be
/// empty, but the API call must succeed (or return a graceful typed error) and
/// `get_all_flights()` must return an empty `Vec` rather than panicking.
#[tokio::test]
#[ignore = "requires live network"]
async fn train_station_iata_returns_empty_or_graceful() -> Result<()> {
    require_live!();
    let client = shared_client().await;

    // XRJ = Rome Termini rail code; XVQ = Venice Santa Lucia rail code.
    let config = Config::builder()
        .departure("XRJ", client)
        .await?
        .destination("XVQ", client)
        .await?
        .departing_date(days_from_now(14))
        .build()?;

    // The request must not panic; either an Ok (possibly empty) or a typed Err.
    match client.request_flights(&config).await {
        Ok(r) => {
            // Accessing get_all_flights() must not panic even for empty results.
            let _ = r.get_all_flights();
        }
        Err(_) => {
            // A graceful typed error is also an acceptable outcome.
        }
    }

    Ok(())
}
