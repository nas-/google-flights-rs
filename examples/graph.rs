use std::cmp::Ordering;

use anyhow::{Context, Result};
use chrono::{Duration, Months, Utc};

use gflights::requests::{
    api::ApiClient,
    config::{Config, Currency},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing. Set RUST_LOG=gflights=debug (or =trace) to see
    // detailed library logs; defaults to INFO if the env var is absent.
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gflights=info")),
        )
        .init();

    // Currency is a client property — it applies to every request.
    let client = ApiClient::new().await.with_currency(Currency::USDollar);

    let today = Utc::now().date_naive();
    let departing_date = today + Duration::days(10);
    let return_date = today + Duration::days(20);

    //Set currency to USDollar, default is euros.
    let config: Config = Config::builder()
        .departure("MAD", &client)
        .await
        .with_context(|| "Failed to set departure airport")?
        .destination("MEX", &client)
        .await
        .with_context(|| "Failed to set destination airport")?
        .departing_date(departing_date)
        .return_date(return_date)
        .build()
        .with_context(|| "Failed to build configuration")?;

    let months = Months::new(5);
    let response = client
        .request_graph(&config, months)
        .await
        .with_context(|| "Failed to request flight data")?;

    let graphs = response.get_all_graphs();

    let lowest_cost = graphs
        .iter()
        .filter_map(|graph| graph.maybe_get_date_price())
        .min_by(|a, b| match a.1.partial_cmp(&b.1) {
            Some(ordering) => ordering,
            None => Ordering::Equal,
        });

    // Display the result.
    if let Some((departure_date, price)) = lowest_cost {
        println!(
            "Lowest cost itinerary: Departure on {}, Price: {:.2} {:?}",
            departure_date.format("%Y-%m-%d"),
            price,
            client.currency()
        );
    } else {
        println!("No prices found for this itinerary.");
    }

    Ok(())
}
