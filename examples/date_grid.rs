use anyhow::{Context, Result};
use chrono::{Duration, Utc};

use gflights::requests::{api::ApiClient, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gflights=info")),
        )
        .init();

    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    // Reference dates used inside the itinerary (should fall within each window).
    let dep_ref = today + Duration::days(10);
    let ret_ref = today + Duration::days(17);

    let config = Config::builder()
        .departure("MAD", &client)
        .await
        .with_context(|| "Failed to set departure airport")?
        .destination("BCN", &client)
        .await
        .with_context(|| "Failed to set destination airport")?
        .departing_date(dep_ref)
        .return_date(ret_ref)
        .build()
        .with_context(|| "Failed to build configuration")?;

    // Search a 7-day departure window and a 7-day return window.
    let dep_start = today + Duration::days(7);
    let dep_end = today + Duration::days(13);
    let ret_start = today + Duration::days(15);
    let ret_end = today + Duration::days(21);

    let grid_response = client
        .request_date_grid(&config, dep_start, dep_end, ret_start, ret_end)
        .await
        .with_context(|| "Failed to request date grid")?;

    println!(
        "Received {} price entries.\n",
        grid_response.entries.len()
    );

    // Print the cheapest option.
    if let Some(best) = grid_response.cheapest() {
        println!(
            "Cheapest: depart {} → return {}  =  {} {:?}",
            best.departure_date, best.return_date, best.price, config.currency
        );
    } else {
        println!("No prices found.");
        return Ok(());
    }

    // Print the grid as a table: rows = departure dates, cols = return dates.
    let grid = grid_response.grid();
    let mut dep_dates: Vec<_> = grid.keys().copied().collect();
    dep_dates.sort();

    // Collect all return dates seen across any departure.
    let mut ret_dates: Vec<_> = grid
        .values()
        .flat_map(|m| m.keys().copied())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    ret_dates.sort();

    // Header row.
    print!("{:<12}", "dep \\ ret");
    for r in &ret_dates {
        print!("{:>12}", r.format("%m-%d").to_string());
    }
    println!();

    for dep in &dep_dates {
        print!("{:<12}", dep.format("%m-%d").to_string());
        for ret in &ret_dates {
            let cell = grid
                .get(dep)
                .and_then(|m| m.get(ret))
                .map(|p| format!("{p}"))
                .unwrap_or_else(|| "-".to_string());
            print!("{:>12}", cell);
        }
        println!();
    }

    Ok(())
}
