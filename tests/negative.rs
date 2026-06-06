//! Negative / error-input tests.
//!
//! Every case here verifies that the library returns a well-formed `Err` (or a
//! well-formed empty `Ok`) rather than panicking, producing garbage output, or
//! silently accepting bad inputs.
//!
//! No network calls are made — all parser tests use inline strings.
//!
//! Run with: `cargo test --test negative`

use chrono::NaiveDate;
use gflights::parsers::common::{AirlineCode, AirlineFilter, Travelers};
use gflights::parsers::response::date_grid_response::parse_date_grid_response;
use gflights::parsers::response::explore_response::parse_explore_response;
use gflights::parsers::response::flight_response::create_raw_response_vec;
use gflights::requests::config::explore::{mid_from_name, region_from_name};
use gflights::requests::config::Config;

// ---------------------------------------------------------------------------
// Travelers validation
// ---------------------------------------------------------------------------

#[test]
fn travelers_zero_adults_is_rejected() {
    let err = Travelers::new(vec![0, 1, 0, 0]).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("adult"),
        "error should mention 'adult', got: {msg}"
    );
}

#[test]
fn travelers_negative_adults_is_rejected() {
    let err = Travelers::new(vec![-1, 0, 0, 0]).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("adult"),
        "error should mention 'adult', got: {msg}"
    );
}

#[test]
fn travelers_too_few_elements_is_rejected() {
    assert!(
        Travelers::new(vec![1, 0, 0]).is_err(),
        "3 elements should fail"
    );
    assert!(Travelers::new(vec![]).is_err(), "empty vec should fail");
}

#[test]
fn travelers_too_many_elements_is_rejected() {
    assert!(
        Travelers::new(vec![1, 0, 0, 0, 0]).is_err(),
        "5 elements should fail"
    );
}

#[test]
fn travelers_over_9_passengers_is_rejected() {
    let err = Travelers::new(vec![5, 5, 0, 0]).unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("9") || msg.contains("exceed") || msg.contains("passenger"),
        "error should mention the limit, got: {msg}"
    );
}

#[test]
fn travelers_exactly_9_is_accepted() {
    assert!(Travelers::new(vec![9, 0, 0, 0]).is_ok());
    assert!(Travelers::new(vec![1, 4, 2, 2]).is_ok()); // 1+4+2+2 = 9
}

#[test]
fn travelers_10_passengers_is_rejected() {
    assert!(Travelers::new(vec![10, 0, 0, 0]).is_err());
}

// ---------------------------------------------------------------------------
// ConfigBuilder validation
// ---------------------------------------------------------------------------

fn lux() -> gflights::parsers::common::Location {
    use gflights::parsers::common::{Location, PlaceType};
    Location {
        loc_identifier: "LUX".into(),
        loc_type: PlaceType::Airport,
        location_name: Some("Luxembourg".into()),
    }
}

fn mxp() -> gflights::parsers::common::Location {
    use gflights::parsers::common::{Location, PlaceType};
    Location {
        loc_identifier: "MXP".into(),
        loc_type: PlaceType::Airport,
        location_name: Some("Milan Malpensa".into()),
    }
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[test]
fn config_build_missing_date_is_rejected() {
    let err = Config::builder()
        .departure_location(lux())
        .destination_location(mxp())
        .build()
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("date") || msg.contains("departing"),
        "error should mention date, got: {msg}"
    );
}

#[test]
fn config_build_missing_departure_is_rejected() {
    let err = Config::builder()
        .departing_date(today())
        .destination_location(mxp())
        .build()
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("departure") || msg.contains("airport") || msg.contains("origin"),
        "error should mention departure, got: {msg}"
    );
}

#[test]
fn config_build_missing_destination_is_rejected() {
    let err = Config::builder()
        .departing_date(today())
        .departure_location(lux())
        .build()
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("destination") || msg.contains("airport"),
        "error should mention destination, got: {msg}"
    );
}

#[test]
fn config_build_valid_minimal_succeeds() {
    Config::builder()
        .departing_date(today())
        .departure_location(lux())
        .destination_location(mxp())
        .build()
        .expect("minimal config with date + departure + destination should succeed");
}

// ---------------------------------------------------------------------------
// AirlineCode validation
// ---------------------------------------------------------------------------

#[test]
fn airline_code_empty_string_is_rejected() {
    assert!(AirlineCode::new("").is_err());
}

#[test]
fn airline_code_one_letter_is_rejected() {
    assert!(AirlineCode::new("A").is_err());
}

#[test]
fn airline_code_three_letters_is_rejected() {
    assert!(
        AirlineCode::new("LHR").is_err(),
        "3-letter airport code should be rejected as airline"
    );
}

#[test]
fn airline_code_with_digit_is_rejected() {
    // Some carriers have a digit (e.g. "B6"), but our validator requires pure letters.
    assert!(AirlineCode::new("B6").is_err());
}

#[test]
fn airline_code_with_spaces_is_rejected() {
    assert!(AirlineCode::new("L X").is_err());
}

#[test]
fn airline_filter_from_str_unknown_code_is_rejected() {
    // A 3-letter value that isn't an alliance name.
    assert!("XYZ".parse::<AirlineFilter>().is_err());
}

#[test]
fn airline_filter_from_str_empty_is_rejected() {
    assert!("".parse::<AirlineFilter>().is_err());
}

// ---------------------------------------------------------------------------
// Flight response parser — malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn flight_parser_empty_string_is_graceful() {
    // Empty body: outer decode fails but should not panic.
    let result = create_raw_response_vec(String::new());
    // Either Err (outer parse failed) or Ok with empty flights — both acceptable.
    if let Ok(container) = result {
        assert!(
            container.get_all_flights().is_empty(),
            "empty body → no flights"
        );
    }
}

#[test]
fn flight_parser_garbage_bytes_is_graceful() {
    let result = create_raw_response_vec("not json at all !!!@#$".to_string());
    if let Ok(container) = result {
        assert!(container.get_all_flights().is_empty());
    }
}

#[test]
fn flight_parser_only_header_line_is_graceful() {
    // Valid wrb.fr header but no actual data.
    let body = ")]}'\n\n[[\"noop\"]]\n";
    let result = create_raw_response_vec(body.to_string());
    if let Ok(container) = result {
        assert!(container.get_all_flights().is_empty());
    }
}

#[test]
fn flight_parser_truncated_json_is_graceful() {
    // Truncated after the first brace — should not panic.
    let result = create_raw_response_vec(")]}'".to_string());
    if let Ok(container) = result {
        assert!(container.get_all_flights().is_empty());
    }
}

#[test]
fn flight_parser_wrong_json_type_is_graceful() {
    // Valid JSON but wrong shape (object instead of array).
    let result = create_raw_response_vec(r#")]}'\n{"key": "value"}"#.to_string());
    if let Ok(container) = result {
        assert!(container.get_all_flights().is_empty());
    }
}

// ---------------------------------------------------------------------------
// Date-grid response parser — malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn date_grid_empty_body_is_graceful() {
    let result = parse_date_grid_response("");
    // Should not panic; either Err or Ok(empty).
    if let Ok(resp) = result {
        assert!(resp.entries.is_empty());
    }
}

#[test]
fn date_grid_garbage_body_is_graceful() {
    let result = parse_date_grid_response("garbage!!!");
    if let Ok(resp) = result {
        assert!(resp.entries.is_empty());
    }
}

#[test]
fn date_grid_json_object_not_array_is_graceful() {
    let result = parse_date_grid_response(r#")]}'\n{"not":"an array"}"#);
    if let Ok(resp) = result {
        assert!(resp.entries.is_empty());
    }
}

#[test]
fn date_grid_minimal_valid_header_returns_empty() {
    // wrb.fr header + noop chunk — no entries.
    let body = ")]}'\n\n[[\"noop\"]]\n";
    let result = parse_date_grid_response(body);
    if let Ok(resp) = result {
        assert!(resp.entries.is_empty(), "noop chunk → 0 entries");
    }
}

// ---------------------------------------------------------------------------
// Explore response parser — malformed inputs
// ---------------------------------------------------------------------------

#[test]
fn explore_empty_body_returns_empty_vec() {
    // Documented to return empty on structural failure.
    let results = parse_explore_response("").unwrap();
    assert!(results.is_empty());
}

#[test]
fn explore_garbage_body_returns_empty_vec() {
    let results = parse_explore_response("not even close to valid").unwrap();
    assert!(results.is_empty());
}

#[test]
fn explore_noop_chunk_returns_empty_vec() {
    let body = ")]}'\n\n[[\"noop\"]]\n";
    let results = parse_explore_response(body).unwrap();
    assert!(results.is_empty());
}

#[test]
fn explore_chunk_with_missing_payload_returns_empty_vec() {
    // Outer array entry with null at position [2] (where the payload lives).
    let body = ")]}'\n\n[[\"wrb.fr\", null, null]]\n";
    let results = parse_explore_response(body).unwrap();
    assert!(results.is_empty());
}

#[test]
fn explore_chunk_with_empty_payload_returns_empty_vec() {
    let body = ")]}'\n\n[[\"wrb.fr\", null, \"\"]]\n";
    let results = parse_explore_response(body).unwrap();
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// Interest / region name resolution — unknown inputs
// ---------------------------------------------------------------------------

#[test]
fn mid_from_name_unknown_string_returns_none() {
    assert_eq!(mid_from_name("surfing"), None);
    assert_eq!(mid_from_name("zorbing"), None);
    assert_eq!(mid_from_name(""), None);
    assert_eq!(mid_from_name("   "), None); // whitespace
}

#[test]
fn mid_from_name_raw_mid_returns_none() {
    // Raw MIDs are handled by the caller layer; this function returns None for them.
    assert_eq!(mid_from_name("/m/01rwk"), None);
    assert_eq!(mid_from_name("/g/11bc58l13w"), None);
}

#[test]
fn mid_from_name_known_aliases_resolve() {
    use gflights::requests::config::explore::Interest;
    assert_eq!(mid_from_name("beaches"), Some(Interest::BEACHES));
    assert_eq!(mid_from_name("beach"), Some(Interest::BEACHES));
    assert_eq!(mid_from_name("BEACHES"), Some(Interest::BEACHES)); // case-insensitive
    assert_eq!(mid_from_name("climbing"), Some(Interest::CLIMBING));
    assert_eq!(mid_from_name("Rock Climbing"), Some(Interest::CLIMBING));
    assert_eq!(mid_from_name("skiing"), Some(Interest::SKIING));
    assert_eq!(mid_from_name("ski"), Some(Interest::SKIING));
    assert_eq!(mid_from_name("museums"), Some(Interest::MUSEUMS));
    assert_eq!(mid_from_name("history"), Some(Interest::HISTORY));
    assert_eq!(mid_from_name("outdoors"), Some(Interest::OUTDOORS));
}

#[test]
fn region_from_name_unknown_string_returns_none() {
    assert_eq!(region_from_name("caribbean"), None);
    assert_eq!(region_from_name("patagonia"), None);
    assert_eq!(region_from_name(""), None);
}

#[test]
fn region_from_name_raw_mid_returns_none() {
    assert_eq!(region_from_name("/m/01531v"), None);
}

#[test]
fn region_from_name_known_aliases_resolve() {
    use gflights::requests::config::explore::Region;
    assert_eq!(region_from_name("alps"), Some(Region::ALPS));
    assert_eq!(region_from_name("Alpine"), Some(Region::ALPS));
    assert_eq!(region_from_name("ALPS"), Some(Region::ALPS));
    assert_eq!(
        region_from_name("northern europe"),
        Some(Region::NORTHERN_EUROPE)
    );
    assert_eq!(
        region_from_name("scandinavia"),
        Some(Region::NORTHERN_EUROPE)
    );
}
