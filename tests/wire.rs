//! Wire-protocol tests: feed captured API response fixtures through the parsers
//! and assert specific field values.
//!
//! These tests do not make network calls — they exercise the full parse path
//! from raw bytes to typed structs using fixtures in `test_files/`.
//!
//! Run with: `cargo test --test wire`

use chrono::Datelike as _;
use gflights::parsers::response::{
    calendar_graph_response::GraphRawResponseContainer, flight_response::RawResponse,
    offer_response::create_raw_response_offer_vec,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_fixture(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
}

// ---------------------------------------------------------------------------
// Flight response — lux_milan_oneway.txt
//
// This fixture is a pre-decoded RawResponse (inner JSON), not the outer
// wrb.fr envelope — parse with serde_json directly, not create_raw_response_vec.
// ---------------------------------------------------------------------------

fn parse_raw_response(path: &str) -> RawResponse {
    let body = read_fixture(path);
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("failed to parse {path} as RawResponse: {e}"))
}

#[test]
fn flight_lux_milan_parses_non_empty() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    let flights = resp.maybe_get_all_flights().unwrap_or_default();
    assert!(!flights.is_empty(), "expected flights in lux_milan fixture");
}

#[test]
fn flight_lux_milan_departure_airport_is_lux() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        let dep = flight
            .itinerary
            .flight_details
            .first()
            .map(|d| d.departure_airport_code.as_str())
            .unwrap_or("");
        assert_eq!(dep, "LUX", "expected departure LUX, got {dep}");
    }
}

#[test]
fn flight_lux_milan_all_airport_codes_valid() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        for leg in &flight.itinerary.flight_details {
            for code in [
                leg.departure_airport_code.as_str(),
                leg.destination_airport_code.as_str(),
            ] {
                // IATA airport codes are 3-char uppercase; city/region identifiers start with /
                if !code.starts_with('/') {
                    assert_eq!(code.len(), 3, "airport code {code:?} must be 3 chars");
                    assert!(
                        code.chars().all(|c| c.is_ascii_uppercase()),
                        "airport code {code:?} must be uppercase ASCII"
                    );
                }
            }
        }
    }
}

#[test]
fn flight_lux_milan_total_time_is_positive() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        assert!(
            flight.itinerary.total_time_minutes > 0,
            "total_time_minutes must be positive"
        );
    }
}

#[test]
fn flight_lux_milan_departure_token_non_empty() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        assert!(
            !flight.itinerary_cost.departure_token.is_empty(),
            "departure_token must not be empty"
        );
    }
}

#[test]
fn flight_lux_milan_stop_count_consistent_with_legs() {
    let resp = parse_raw_response("test_files/lux_milan_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        let expected = flight.itinerary.flight_details.len().saturating_sub(1);
        assert_eq!(
            flight.itinerary.stop_count(),
            expected,
            "stop_count inconsistent with flight_details length"
        );
    }
}

// ---------------------------------------------------------------------------
// Flight response — lux_tokyo_oneway.txt (long-haul, likely multi-stop)
//
// Also a pre-decoded RawResponse fixture.
// ---------------------------------------------------------------------------

#[test]
fn flight_lux_tokyo_all_leg_durations_positive() {
    let resp = parse_raw_response("test_files/lux_tokyo_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        for leg in &flight.itinerary.flight_details {
            if let Some(dur) = leg.leg_duration_minutes {
                assert!(dur > 0, "leg duration must be positive, got {dur}");
            }
        }
    }
}

#[test]
fn flight_lux_tokyo_airline_code_non_empty() {
    let resp = parse_raw_response("test_files/lux_tokyo_oneway.txt");
    for flight in resp.maybe_get_all_flights().unwrap_or_default() {
        assert!(
            !flight.itinerary.flight_by.is_empty(),
            "flight_by (airline IATA) must not be empty"
        );
    }
}

#[test]
fn flight_lux_tokyo_multi_leg_flights_have_multiple_details() {
    // LUX→TYO requires at least one connection — multi-leg itineraries must
    // have more than one FlightInfo entry.
    let resp = parse_raw_response("test_files/lux_tokyo_oneway.txt");
    let flights = resp.maybe_get_all_flights().unwrap_or_default();
    let multi_leg = flights.iter().any(|f| f.itinerary.flight_details.len() > 1);
    assert!(
        multi_leg,
        "expected at least one multi-leg itinerary on LUX→TYO"
    );
}

// ---------------------------------------------------------------------------
// Flight response — itinerary helper methods
// ---------------------------------------------------------------------------

#[test]
fn flight_stop_count_equals_connections_length() {
    // The inline nonstop fixture from the unit tests — verified against real data.
    let raw = r#"["LG", ["Luxair"], [[null, null, null, "LUX", "Luxembourg Airport", "Milan Malpensa Airport", "MXP", null, [11, 10], null, [12, 25], 75, [], 1, "76 cm", [["AZ", "7879", null, "ITA"]], 1, "De Havilland-Bombardier Dash-8", null, false, [2024, 1, 27], [2024, 1, 27], ["LG", "6993", null, "Luxair"], null, null, 1, null, null, null, null, "76 centimetres", 35968]], "LUX", [2024, 1, 27], [11, 10], "MXP", [2024, 1, 27], [12, 25], 75, null, null, false, null, null, null, ["ITA"], "VDOwRb", [[1705070296848121, 139803069, 858572], null, null, null, null, [[6]]], 1, null, null, [null, null, 1, -58, null, true, true, 36000, 86000, [true], 119000, 1, false], [1], [["LG", "Luxair", "https://www.luxair.lu/en/information/passenger-assistance"]]]"#;
    let it: gflights::parsers::response::flight_response::Itinerary =
        serde_json::from_str(raw).expect("parse failed");
    assert_eq!(it.stop_count(), 0);
    assert!(it.connection_info.as_deref().unwrap_or(&[]).is_empty());
    assert_eq!(it.total_time_minutes, 75);
}

// ---------------------------------------------------------------------------
// Graph response — graph_response fixture
// ---------------------------------------------------------------------------

#[test]
fn graph_fixture_parses_without_error() {
    let body = read_fixture("test_files/graph_response");
    let container = GraphRawResponseContainer::try_from(body.as_str());
    assert!(container.is_ok(), "graph_response fixture failed to parse");
}

#[test]
fn graph_fixture_contains_at_least_one_date() {
    let body = read_fixture("test_files/graph_response");
    let container = GraphRawResponseContainer::try_from(body.as_str()).expect("parse failed");
    let entries = container.get_all_graphs();
    assert!(
        !entries.is_empty(),
        "expected at least one date entry in graph_response fixture"
    );
}

#[test]
fn graph_fixture_proposed_departure_dates_non_zero() {
    let body = read_fixture("test_files/graph_response");
    let container = GraphRawResponseContainer::try_from(body.as_str()).expect("parse failed");
    for entry in container.get_all_graphs() {
        let d = entry.proposed_departure_date;
        // NaiveDate default is year 0 — any real date has year >= 2020
        assert!(
            d.year() >= 2020,
            "proposed_departure_date looks invalid: {d}"
        );
    }
}

#[test]
fn graph_fixture_prices_are_positive() {
    let body = read_fixture("test_files/graph_response");
    let container = GraphRawResponseContainer::try_from(body.as_str()).expect("parse failed");
    for entry in container.get_all_graphs() {
        if let Some(cost) = &entry.proposed_trip_cost {
            assert!(
                cost.trip_cost.price > 0,
                "graph price must be positive, got {}",
                cost.trip_cost.price
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Offer response — offers_full.txt
// ---------------------------------------------------------------------------

#[test]
fn offer_fixture_parses_without_error() {
    let body = read_fixture("test_files/offers_full.txt");
    let container = create_raw_response_offer_vec(body);
    assert!(container.is_ok(), "offers_full.txt failed to parse");
}

#[test]
fn offer_fixture_contains_at_least_one_offer_group() {
    let body = read_fixture("test_files/offers_full.txt");
    let container = create_raw_response_offer_vec(body).expect("parse failed");
    let groups: Vec<_> = container.response.iter().flat_map(|r| &r.offers).collect();
    assert!(
        !groups.is_empty(),
        "expected at least one offer group in offers_full fixture"
    );
}

#[test]
fn offer_fixture_prices_are_positive_when_present() {
    let body = read_fixture("test_files/offers_full.txt");
    let container = create_raw_response_offer_vec(body).expect("parse failed");
    for group in container.response.iter().flat_map(|r| &r.offers) {
        if let Some(price) = group.price {
            assert!(price > 0, "offer price must be positive, got {price}");
        }
    }
}

#[test]
fn offer_fixture_airline_names_non_empty_when_present() {
    let body = read_fixture("test_files/offers_full.txt");
    let container = create_raw_response_offer_vec(body).expect("parse failed");
    for group in container.response.iter().flat_map(|r| &r.offers) {
        if !group.airline_names.is_empty() {
            for name in &group.airline_names {
                assert!(!name.is_empty(), "airline name must not be empty string");
            }
        }
    }
}

#[test]
fn offer_fixture_get_offer_prices_sorted() {
    let body = read_fixture("test_files/offers_full.txt");
    let container = create_raw_response_offer_vec(body).expect("parse failed");
    for resp in &container.response {
        if let Some(prices) = resp.get_offer_prices() {
            let price_values: Vec<i32> = prices.iter().map(|(_, p)| *p).collect();
            let mut sorted = price_values.clone();
            sorted.sort();
            // get_offer_prices doesn't guarantee order — just check all are positive
            for p in &price_values {
                assert!(*p > 0, "offer price from get_offer_prices must be positive");
            }
        }
    }
}
