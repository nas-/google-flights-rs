use anyhow::{Context, Result};
use chrono::{Duration, Utc, Weekday};

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
        .departure("LUX", &client)
        .await
        .with_context(|| "Failed to set departure airport")?
        .destination("BCN", &client)
        .await
        .with_context(|| "Failed to set destination airport")?
        .departing_date(dep_ref)
        .return_date(ret_ref)
        .build()
        .with_context(|| "Failed to build configuration")?;

    // Wide windows — the library splits them into ≤200-cell requests automatically.
    let dep_start = today + Duration::days(6);
    let dep_end = today + Duration::days(15);
    let ret_start = today + Duration::days(10);
    let ret_end = today + Duration::days(40);

    let grid_response = client
        .request_date_grid(&config, dep_start, dep_end, ret_start, ret_end)
        .await
        .with_context(|| "Failed to request date grid")?;

    println!("Received {} price entries.\n", grid_response.entries.len());

    // --- Cheapest of any combination ----------------------------------------
    if let Some(best) = grid_response.cheapest() {
        println!(
            "Cheapest overall:  depart {} ({})  →  return {} ({})  =  {} {:?}",
            best.departure_date,
            best.departure_date.format("%a"),
            best.return_date,
            best.return_date.format("%a"),
            best.price,
            config.currency,
        );
    } else {
        println!("No prices found.");
        return Ok(());
    }

    // --- Cheapest weekend trip -----------------------------------------------
    // "Weekend" here means: leave Friday or Saturday, come back Sunday or Monday.
    // Time-of-day preferences (e.g. afternoon departure) are handled server-side
    // by setting departing_times / return_times in Config before the request.
    let weekend_combos = [
        (Weekday::Fri, Weekday::Sun),
        (Weekday::Fri, Weekday::Mon),
        (Weekday::Sat, Weekday::Sun),
        (Weekday::Sat, Weekday::Mon),
    ];
    if let Some(best) = grid_response.cheapest_for_weekdays(&weekend_combos) {
        println!(
            "Cheapest weekend:  depart {} ({})  →  return {} ({})  =  {} {:?}",
            best.departure_date,
            best.departure_date.format("%a"),
            best.return_date,
            best.return_date.format("%a"),
            best.price,
            config.currency,
        );
    } else {
        println!("No weekend options found in this window.");
    }

    // --- All Friday → Sunday options, sorted by price ------------------------
    println!("\nAll Friday → Sunday options:");
    let mut fri_sun: Vec<_> = grid_response
        .filter_weekdays(Weekday::Fri, Weekday::Sun)
        .collect();
    fri_sun.sort_by_key(|e| e.price);
    if fri_sun.is_empty() {
        println!("  (none)");
    }
    for e in &fri_sun {
        println!(
            "  {} → {}  =  {} {:?}",
            e.departure_date, e.return_date, e.price, config.currency
        );
    }

    // --- Full price table ----------------------------------------------------
    println!("\nFull price grid (rows = departure, cols = return):\n");
    let grid = grid_response.grid();
    let mut dep_dates: Vec<_> = grid.keys().copied().collect();
    dep_dates.sort();
    let mut ret_dates: Vec<_> = grid
        .values()
        .flat_map(|m| m.keys().copied())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    ret_dates.sort();

    print!("{:<14}", "dep \\ ret");
    for r in &ret_dates {
        print!("{:>10}", r.format("%m-%d(%a)").to_string());
    }
    println!();

    for dep in &dep_dates {
        print!("{:<14}", dep.format("%m-%d(%a)").to_string());
        for ret in &ret_dates {
            let cell = grid
                .get(dep)
                .and_then(|m| m.get(ret))
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            print!("{:>10}", cell);
        }
        println!();
    }

    Ok(())
}
