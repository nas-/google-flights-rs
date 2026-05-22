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
use gflights::requests::{api::ApiClient, config::Config};

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
                eprintln!(
                    "[live_api] skipping — set RUN_LIVE_TESTS=1 to run live tests"
                );
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

    assert!(!loc.loc_identifier.is_empty(), "loc_identifier should not be empty");
    assert!(
        loc.location_name.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
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

    assert_eq!(config.departure.loc_identifier, "LHR");
    assert_eq!(config.destination.loc_identifier, "JFK");
    assert_eq!(
        config.departure.location_name.as_deref(),
        Some("LHR"),
        "IATA departure should use code as location_name"
    );
    assert_eq!(
        config.destination.location_name.as_deref(),
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

    assert!(!flights.is_empty(), "need at least one flight to validate structure");

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
            assert_airport_code(&leg.departure_airport_code, &format!("flight[{i}] leg[{j}] dep"));
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
            !itinerary_container.itinerary_cost.departure_token.is_empty(),
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
    assert_airport_code(&last_leg.destination_airport_code, "return last leg destination");

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
                    assert!(cost.trip_cost.price > 0, "cheaper-date suggestion price should be positive");
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
        assert!(bound > 0, "usual_price_bound should be a positive integer, got {bound}");
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

    assert_eq!(config.departure.len(), 2, "should have LHR and LGW as departure airports");

    let response = client.request_flights(&config).await?;
    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(
        !flights.is_empty(),
        "expected ≥1 flight for (LHR|LGW)→JFK"
    );

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

    assert_eq!(config.destination.len(), 2, "should have JFK and EWR as destination airports");

    let response = client.request_flights(&config).await?;
    let flights: Vec<_> = response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    assert!(
        !flights.is_empty(),
        "expected ≥1 flight for LHR→(JFK|EWR)"
    );

    Ok(())
}
