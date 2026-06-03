//! Example: fetch booking offers and resolve a deep-link URL.
//!
//! Shows the two-step flow: search for flights, pick one, then call
//! [`ApiClient::request_offer`] to get airline-specific prices and booking
//! links, and [`ApiClient::resolve_booking_url`] to turn a click token into a
//! real URL.
//!
//! Run with live network access:
//!   RUN_LIVE=1 cargo run --example offer
//!
//! Without RUN_LIVE the example exits early with a message so that `cargo test`
//! does not perform network requests.

use anyhow::Context;
use chrono::{Duration, Utc};
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
    let today = Utc::now().date_naive();

    // -------------------------------------------------------------------------
    // Step 1: search for flights and pick the first result
    // -------------------------------------------------------------------------
    let config = Config::builder()
        .departure("LHR", &client)
        .await
        .context("departure lookup")?
        .destination("JFK", &client)
        .await
        .context("destination lookup")?
        .departing_date(today + Duration::days(30))
        .build()
        .context("build config")?;

    let search = client
        .request_flights(&config)
        .await
        .context("flight search")?;

    let first = search
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .context("no flights found")?;

    println!(
        "Selected flight: {} · {} min · {} stop(s)",
        first.itinerary.flight_by,
        first.itinerary.total_time_minutes,
        first.itinerary.stop_count(),
    );

    // Lock in the selected flight so the offer request knows which itinerary
    // we want offers for.
    config.fixed_flights.add_element(first)?;

    // -------------------------------------------------------------------------
    // Step 2: request booking offers for that itinerary
    // -------------------------------------------------------------------------
    let offers_response = client
        .request_offer(&config)
        .await
        .context("offer request")?;

    let mut offer_groups: Vec<_> = offers_response
        .response
        .iter()
        .flat_map(|r| &r.offers)
        .filter(|o| o.price.is_some())
        .collect();

    offer_groups.sort_by_key(|o| o.price.unwrap_or(i32::MAX));

    if offer_groups.is_empty() {
        println!("No offers returned for this itinerary.");
        return Ok(());
    }

    println!("\nBooking offers (sorted cheapest first):");

    for offer in &offer_groups {
        let price = offer.price.unwrap_or(0);
        let airlines = offer.airline_names.join(", ");
        println!("  {airlines}  →  {price} EUR");

        // -------------------------------------------------------------------------
        // Step 3: resolve a click token to a real booking URL
        // -------------------------------------------------------------------------
        if let Some(token) = offer.click_token.as_deref() {
            match client.resolve_booking_url(token).await {
                Ok(url) => println!("    URL: {url}"),
                Err(e) => println!("    (URL resolution failed: {e})"),
            }
        } else {
            // Some offers have sub-options (e.g. different travel agencies).
            for sub in offer.sub_options.iter().take(2) {
                let sub_price = sub.price.map(|p| format!("{p} EUR")).unwrap_or_default();
                let partners = sub.partner_names.join(", ");
                if let Some(token) = sub.click_token.as_deref() {
                    match client.resolve_booking_url(token).await {
                        Ok(url) => println!("    [{partners}  {sub_price}]  {url}"),
                        Err(e) => println!("    [{partners}] (failed: {e})"),
                    }
                }
            }
        }
    }

    Ok(())
}
