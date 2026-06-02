//! Multi-city (open-jaw) search example.
//!
//! Set RUN_LIVE to actually call the API; otherwise it prints the config and exits.
//!
//! ```sh
//! RUN_LIVE=1 cargo run --example multi_city
//! ```

use chrono::NaiveDate;
use gflights::requests::api::ApiClient;
use gflights::requests::config::MultiCityConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ApiClient::new().await;
    let config = MultiCityConfig::builder()
        .add_leg(
            "LUX",
            "FCO",
            NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
            &client,
        )
        .await?
        .add_leg(
            "FCO",
            "MAD",
            NaiveDate::from_ymd_opt(2026, 9, 13).unwrap(),
            &client,
        )
        .await?
        .add_leg(
            "MAD",
            "LUX",
            NaiveDate::from_ymd_opt(2026, 9, 17).unwrap(),
            &client,
        )
        .await?
        .build()?;

    if std::env::var("RUN_LIVE").is_err() {
        println!("Config built: {} legs", config.legs.len());
        println!("Set RUN_LIVE=1 to run the search.");
        return Ok(());
    }

    let results = client.request_multi_city_flights(&config).await?;
    let flights = results.get_all_flights();
    println!("Found {} flight options", flights.len());
    for f in flights.iter().take(5) {
        println!(
            "  {} — {:?}",
            f.itinerary.flight_by,
            f.itinerary_cost.trip_cost.as_ref().map(|c| c.price)
        );
    }
    Ok(())
}
