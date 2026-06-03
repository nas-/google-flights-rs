//! Example: explore cheap destinations from a given origin.
//!
//! Run with live network access:
//!   RUN_LIVE=1 cargo run --example explore
//!
//! Without RUN_LIVE the example exits early with a message so that `cargo test`
//! does not perform network requests.

use gflights::parsers::common::{Location, PlaceType, TravelClass};
use gflights::requests::api::ApiClient;
use gflights::requests::config::explore::{ExploreConfig, ExploreDate, ExploreDuration, Interest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise structured logging; RUST_LOG controls the level.
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

    let config = ExploreConfig {
        // Depart from Luxembourg Airport.
        origin: vec![Location {
            loc_identifier: "LUX".into(),
            loc_type: PlaceType::Airport,
            location_name: None,
        }],
        // Search for trips in July.
        trip_date: Some(ExploreDate { month: 7 }),
        // One-week trips.
        trip_duration: ExploreDuration::OneWeek,
        // Budget up to €500 round-trip.
        max_price: Some(500),
        // Beach destinations.
        interest: Some(Interest::BEACHES.to_string()),
        // Max 5-hour flight time.
        max_flight_duration_minutes: Some(300),
        // Default: economy, 1 adult, EUR, en/GB.
        travel_class: TravelClass::Economy,
        ..Default::default()
    };

    println!("Searching for explore destinations from LUX…");
    let results = client.request_explore(&config).await?;

    if results.is_empty() {
        println!("No destinations returned.");
        return Ok(());
    }

    println!(
        "\n{:<20}  {:<12}  {:<5}  {:>6}  {:<8}  {:>5}  DATES",
        "DESTINATION", "COUNTRY", "ARPT", "PRICE", "AIRLINE", "STOPS"
    );
    println!("{}", "-".repeat(80));

    for r in &results {
        let price_str = r.price.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
        let airline_str = r.airline.as_deref().unwrap_or("—");
        let stops_str = r.stops.map(|s| s.to_string()).unwrap_or_else(|| "—".into());
        let dates_str = match (r.date_from, r.date_to) {
            (Some(f), Some(t)) => format!("{f} → {t}"),
            (Some(f), None) => f.to_string(),
            _ => "—".to_string(),
        };
        println!(
            "{:<20}  {:<12}  {:<5}  {:>6}  {:<8}  {:>5}  {}",
            &r.name[..r.name.len().min(20)],
            &r.country[..r.country.len().min(12)],
            r.nearest_airport,
            price_str,
            airline_str,
            stops_str,
            dates_str,
        );
    }

    println!("\nTotal: {} destinations", results.len());
    Ok(())
}
