//! Example: find the cheapest departure dates for a route.
//!
//! Demonstrates both one-way mode (price calendar) and round-trip mode (date
//! grid) using [`ApiClient::cheapest_dates`].
//!
//! Run with live network access:
//!   RUN_LIVE=1 cargo run --example cheapest_dates
//!
//! Without RUN_LIVE the example exits early with a message so that `cargo test`
//! does not perform network requests.

use chrono::{Months, NaiveDate};
use gflights::parsers::common::{Location, PlaceType};
use gflights::requests::api::ApiClient;
use gflights::requests::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gflights=info")),
        )
        .init();

    if std::env::var("RUN_LIVE").is_err() {
        println!("Set RUN_LIVE=1 to run this example with live network access.");
        return Ok(());
    }

    let client = ApiClient::new().await;

    // Build a config for the LUX → JFK route starting from a fixed date.
    // Using departure_location / destination_location directly avoids an extra
    // city-lookup API call when you already have the IATA code.
    let config = Config::builder()
        .departure_location(Location {
            loc_identifier: "LUX".into(),
            loc_type: PlaceType::Airport,
            location_name: Some("Luxembourg".into()),
        })
        .destination_location(Location {
            loc_identifier: "JFK".into(),
            loc_type: PlaceType::Airport,
            location_name: Some("New York".into()),
        })
        .departing_date(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
        .build()?;

    // -------------------------------------------------------------------------
    // One-way mode: cheapest single dates over the next 3 months
    // -------------------------------------------------------------------------
    println!("=== Cheapest one-way dates (LUX → JFK, next 3 months) ===");
    let oneway = client.cheapest_dates(&config, Months::new(3), None).await?;

    if oneway.is_empty() {
        println!("  No dates found.");
    } else {
        for date in oneway.iter().take(5) {
            println!("  Depart {}  →  {} EUR", date.departure_date, date.price);
        }
        if oneway.len() > 5 {
            println!("  … and {} more", oneway.len() - 5);
        }
    }

    // -------------------------------------------------------------------------
    // Round-trip mode: cheapest 7-night trips over the next 3 months
    // -------------------------------------------------------------------------
    println!();
    println!("=== Cheapest 7-night round trips (LUX ⇄ JFK, next 3 months) ===");
    let roundtrip = client
        .cheapest_dates(&config, Months::new(3), Some(7))
        .await?;

    if roundtrip.is_empty() {
        println!("  No date pairs found.");
    } else {
        for pair in roundtrip.iter().take(5) {
            let ret = pair
                .return_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "?".into());
            println!(
                "  Depart {}  Return {}  →  {} EUR",
                pair.departure_date, ret, pair.price
            );
        }
        if roundtrip.len() > 5 {
            println!("  … and {} more", roundtrip.len() - 5);
        }
    }

    Ok(())
}
