//! Example: User-Agent rotation and override.
//!
//! By default each [`ApiClient`] picks a real desktop browser User-Agent from a
//! rotating pool, so traffic from repeated client creation is not trivially
//! fingerprinted by a single static value. You can also pin a specific
//! User-Agent with [`ApiClient::with_user_agent`].
//!
//! Run with live network access:
//!   RUN_LIVE=1 cargo run --example fingerprint
//!
//! Without RUN_LIVE the example exits early so that `cargo test --examples`
//! does not perform network requests.

use anyhow::Context;
use chrono::{Duration, Utc};
use gflights::requests::api::ApiClient;
use gflights::requests::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUN_LIVE").is_err() {
        println!("Set RUN_LIVE=1 to run this example with live network access.");
        return Ok(());
    }

    // Two independently-constructed clients each pick from the pool.
    let a = ApiClient::new().await;
    let b = ApiClient::new().await;
    println!("client A User-Agent: {}", a.user_agent());
    println!("client B User-Agent: {}", b.user_agent());

    // Pin a specific User-Agent.
    let custom = "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0";
    let pinned = ApiClient::new().await.with_user_agent(custom);
    println!("pinned User-Agent:   {}", pinned.user_agent());
    assert_eq!(pinned.user_agent(), custom);

    // The pinned User-Agent is what goes out on a real request.
    let today = Utc::now().date_naive();
    let config = Config::builder()
        .departure("LHR", &pinned)
        .await
        .context("departure lookup")?
        .destination("JFK", &pinned)
        .await
        .context("destination lookup")?
        .departing_date(today + Duration::days(30))
        .build()
        .context("build config")?;

    let search = pinned
        .request_flights(&config)
        .await
        .context("flight search")?;
    let n: usize = search
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .map(|f| f.len())
        .sum();
    println!("live search with pinned User-Agent returned {n} flights");

    Ok(())
}
