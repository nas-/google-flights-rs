//! Example: route requests through a proxy.
//!
//! [`ApiClient::new_with_proxy`] sends every request — including the one-time
//! frontend-version probe — through the given proxy. Supports `http://`,
//! `https://`, and `socks5://` URLs.
//!
//! Run with live network access and a reachable proxy:
//!   RUN_LIVE=1 PROXY_URL=socks5://127.0.0.1:9050 cargo run --example proxy
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
        println!("Set RUN_LIVE=1 (and PROXY_URL=...) to run this example.");
        return Ok(());
    }

    let proxy = std::env::var("PROXY_URL")
        .context("set PROXY_URL, e.g. http://127.0.0.1:3128 or socks5://127.0.0.1:9050")?;
    println!("routing all requests through proxy: {proxy}");

    let client = ApiClient::new_with_proxy(proxy)
        .await
        .context("failed to build proxied client")?;

    let today = Utc::now().date_naive();
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
    let n: usize = search
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .map(|f| f.len())
        .sum();
    println!("search through proxy returned {n} flights");

    Ok(())
}
