//! Example: find discounted destinations (flight deals) from an origin.
//!
//! Uses the Google Flights deals endpoint (`GetFlightDealsStreaming`). The
//! out/return dates act as a trip-length anchor; the endpoint returns deals of
//! similar length across many dates.
//!
//! Run with live network access:
//!   RUN_LIVE=1 cargo run --example deals
//!
//! Without RUN_LIVE the example exits early so that `cargo test --examples`
//! does not perform network requests.

use anyhow::Context;
use chrono::{Duration, Utc};
use gflights::parsers::common::{Location, PlaceType, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::DealConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUN_LIVE").is_err() {
        println!("Set RUN_LIVE=1 to run this example with live network access.");
        return Ok(());
    }

    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    let config = DealConfig {
        origin: vec![Location {
            loc_identifier: "LUX".to_string(),
            loc_type: PlaceType::Airport,
            location_name: None,
        }],
        outbound_date: today + Duration::days(30),
        return_date: today + Duration::days(34),
        nonstop: false,
        travellers: Travelers::new(vec![1, 0, 0, 0])?,
        ..Default::default()
    };

    let mut deals = client
        .request_deals(&config)
        .await
        .context("deals request")?;
    deals.sort_by_key(|d| std::cmp::Reverse(d.discount_pct));

    if deals.is_empty() {
        println!("No deals found.");
        return Ok(());
    }

    println!("Top deals from LUX (best discount first):");
    for d in deals.iter().take(10) {
        println!(
            "  {:>3}% off  {:>4} (typ {:>4})  {} ({})  {} stop(s)  {}→{}",
            d.discount_pct.unwrap_or(0),
            d.price.unwrap_or(0),
            d.typical_price.unwrap_or(0),
            d.destination_city,
            d.destination_iata,
            d.stops.unwrap_or(0),
            d.outbound_date.map(|x| x.to_string()).unwrap_or_default(),
            d.return_date.map(|x| x.to_string()).unwrap_or_default(),
        );
        if let Some(url) = &d.booking_url {
            println!("           {url}");
        }
    }

    Ok(())
}
