use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use gflights::requests::config::TripType;

use gflights::requests::{api::ApiClient, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    // Log level is controlled by RUST_LOG (e.g. RUST_LOG=gflights=info).
    // Defaults to TRACE so request details are visible without any env-var
    // setup — useful on Windows where inline env-var syntax is awkward.
    // On PowerShell you can override it with:
    //   $env:RUST_LOG="gflights=info"; cargo run --example flights
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gflights=trace")),
        )
        .init();

    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();
    let departing_date = today + Duration::days(10);
    let return_date = today + Duration::days(20);

    let config = Config::builder()
        .departure("MAD", &client)
        .await
        .with_context(|| "Failed to set departure airport")?
        .destination("MEX", &client)
        .await
        .with_context(|| "Failed to set destination airport")?
        .departing_date(departing_date)
        .return_date(return_date)
        .build()
        .with_context(|| "Failed to build the configuration")?;

    // --- Outbound leg --------------------------------------------------------

    let flight_response = client
        .request_flights(&config)
        .await
        .with_context(|| "Failed to request flights")?;

    let first_flight = flight_response
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next()
        .with_context(|| "No flights found")?;

    println!("Outbound: {:?}", first_flight.itinerary);
    println!("Price: {:?} {:?}", first_flight.itinerary_cost.trip_cost, config.currency);
    config.fixed_flights.add_element(first_flight)?;

    // --- Return leg (round-trip only) ----------------------------------------

    if config.trip_type == TripType::Return {
        let return_response = client
            .request_flights(&config)
            .await
            .with_context(|| "Failed to request return flights")?;

        let maybe_return = return_response
            .responses
            .iter()
            .filter_map(|r| r.maybe_get_all_flights())
            .flatten()
            .next();

        if let Some(flight) = maybe_return {
            println!("Return:   {:?}", flight.itinerary);
            println!("Price:    {:?} {:?}", flight.itinerary_cost.trip_cost, config.currency);
            config.fixed_flights.add_element(flight)?;
        } else {
            println!("No return flights found — showing outbound offers only.");
        }
    }

    println!("Itinerary URL: {}", config.to_flight_url());

    // --- Offers --------------------------------------------------------------

    let offers_response = client
        .request_offer(&config)
        .await
        .with_context(|| "Failed to request offers")?;

    let mut offer_groups: Vec<_> = offers_response
        .response
        .iter()
        .flat_map(|r| &r.offers)
        .filter(|o| o.price.is_some())
        .collect();

    offer_groups.sort_by_key(|o| o.price.unwrap_or(i32::MAX));

    if offer_groups.is_empty() {
        println!("No offers found");
        return Ok(());
    }

    for offer in &offer_groups {
        println!(
            "Offer: {:?}  Price: {} {:?}",
            offer.airline_names,
            offer.price.unwrap(),
            config.currency
        );

        if let Some(token) = offer.click_token.as_deref() {
            match client.resolve_booking_url(token).await {
                Ok(url) => println!("  ->  {url}"),
                Err(e) => println!("  (could not resolve URL: {e})"),
            }
        } else if !offer.sub_options.is_empty() {
            for sub in &offer.sub_options {
                if let Some(token) = sub.click_token.as_deref() {
                    let label = if sub.partner_names.is_empty() {
                        "unknown".to_string()
                    } else {
                        sub.partner_names.join(", ")
                    };
                    let price_str = sub
                        .price
                        .map(|p| format!("{} {:?}", p, config.currency))
                        .unwrap_or_default();
                    match client.resolve_booking_url(token).await {
                        Ok(url) => println!("  [{label}  {price_str}]  ->  {url}"),
                        Err(e) => println!("  [{label}] (could not resolve URL: {e})"),
                    }
                }
            }
        } else {
            println!("  (no booking link available)");
        }
    }

    Ok(())
}
