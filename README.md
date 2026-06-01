# gflights

Unofficial async Rust client for the [Google Flights](https://www.google.com/flights) web API.

Search flights, compare prices across a date range, retrieve booking offers, and resolve booking URLs — all without an official API key.

> **Disclaimer:** This library talks to the same endpoints used by the Google Flights website.  It is not affiliated with or endorsed by Google.  Usage is subject to Google's Terms of Service.

---

## Features

- **Flight search** — one-way, return, multi-stop itineraries
- **Price graph** — cheapest fares across a configurable date range
- **Booking offers** — airline/OTA offers with prices and booking URLs
- **City / airport lookup** — resolve city names and IATA codes
- **Multi-airport search** — up to 4 departure or destination airports
- **Locale support** — `language` + `country` for non-English results
- **Sort order** — Best · Price · Duration · Departure time · Arrival time
- **CO2 / emissions** — included in parsed itinerary data
- **Layover details** — connection time, airport codes, overnight warnings
- **Rate limiting** — built-in governor-based token-bucket limiter
- **Retry logic** — exponential back-off for transient 5xx / timeout errors

---

## Installation

```toml
[dependencies]
gflights = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

---

## Quick start

### Search for flights

```rust
use gflights::requests::{api::ApiClient, config::Config};
use chrono::{Duration, Utc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    let config = Config::builder()
        .departure("LHR", &client).await?   // London Heathrow
        .destination("JFK", &client).await? // New York JFK
        .departing_date(today + Duration::days(14))
        .return_date(today + Duration::days(21))
        .build()?;

    let results = client.request_flights(&config).await?;

    for resp in &results.responses {
        if let Some(flights) = resp.maybe_get_all_flights() {
            for f in &flights {
                println!(
                    "{} — {}h{}m — stops: {} — {:?}",
                    f.itinerary.flight_by,
                    f.itinerary.total_time_minutes / 60,
                    f.itinerary.total_time_minutes % 60,
                    f.itinerary.stop_count(),
                    f.itinerary_cost.trip_cost,
                );
            }
        }
    }
    Ok(())
}
```

Run the full worked example:

```sh
cargo run --example flights
```

### Price graph across a date range

```rust
use gflights::requests::{api::ApiClient, config::{Config, Currency}};
use chrono::{Duration, Months, Utc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();

    let config = Config::builder()
        .departure("MAD", &client).await?
        .destination("MEX", &client).await?
        .departing_date(today + Duration::days(10))
        .currency(Currency::USDollar)
        .build()?;

    let graph = client.request_graph(&config, Months::new(3)).await?;

    if let Some((date, price)) = graph
        .get_all_graphs()
        .iter()
        .filter_map(|g| g.maybe_get_date_price())
        .min_by_key(|&(_, p)| p)
    {
        println!("Cheapest: {} at ${:.2}", date, price);
    }
    Ok(())
}
```

Run with:

```sh
cargo run --example graph
```

---

## Configuration reference

| Builder method | Type | Default | Description |
|---|---|---|---|
| `.departure(iata, &client)` | `async &str` | required | Departure airport / city |
| `.destination(iata, &client)` | `async &str` | required | Destination airport / city |
| `.departure_location(loc)` | `Location` | — | Set departure from existing `Location` (no network) |
| `.destination_location(loc)` | `Location` | — | Set destination from existing `Location` |
| `.add_departure(iata, &client)` | `async &str` | — | Add extra departure (max 4) |
| `.add_destination(iata, &client)` | `async &str` | — | Add extra destination (max 4) |
| `.departing_date(date)` | `NaiveDate` | required | Outbound departure date |
| `.return_date(date)` | `NaiveDate` | one-way | Return date (omit for one-way) |
| `.travelers(t)` | `Travelers` | 1 adult | Passenger counts |
| `.travel_class(c)` | `TravelClass` | Economy | Economy / Business / First |
| `.stop_options(s)` | `StopOptions` | Any | Non-stop · Max1 · Any |
| `.sort_order(s)` | `SortOrder` | Best | Best · Price · Duration · DepartureTime · ArrivalTime |
| `.currency(c)` | `Currency` | EUR | Result currency |
| `.language(s)` | `&str` | `"en"` | BCP-47 language subtag |
| `.country(s)` | `&str` | `"GB"` | ISO 3166-1 alpha-2 country code |
| `.stopover_max(d)` | `StopoverDuration` | Unlimited | Maximum layover duration |
| `.duration_max(d)` | `TotalDuration` | Unlimited | Maximum total trip duration |
| `.departing_times(t)` | `FlightTimes` | Any | Outbound departure time window |
| `.return_times(t)` | `FlightTimes` | Any | Return departure time window |

### Travelers

```rust
use gflights::parsers::common::Travelers;

// [adults, children, infants_in_seat, infants_on_lap]
let travelers = Travelers::new(vec![2, 1, 0, 0])?; // 2 adults + 1 child
```

Rules: at least 1 adult, total ≤ 9 passengers.

---

## Rate limiting

`ApiClient` uses a [governor](https://crates.io/crates/governor) token-bucket rate limiter (default: 10 req/s).

```rust
use gflights::requests::api::ApiClient;
use governor::Quota;
use std::num::NonZeroU32;

// Custom: 2 requests per second
let quota = Quota::per_second(NonZeroU32::new(2).unwrap());
let client = ApiClient::new_with_ratelimit(quota).await;
```

If Google returns HTTP 429, the client sets an internal flag and all subsequent requests immediately return `RateLimitedError` without touching the network.  Reset it after a cooling-off period:

```rust
if client.is_rate_limited() {
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    client.reset_rate_limit();
}
```

---

## Retry logic

Transient server errors (HTTP 500/502/503/504) and connection timeouts are automatically retried with exponential back-off.  Defaults: 3 attempts, 500 ms base delay, 30 s cap.

```rust
use gflights::{requests::api::ApiClient, RetryConfig};

let client = ApiClient::new().await
    .with_retry_config(RetryConfig {
        max_attempts: 5,
        base_delay_ms: 200,
        cap_delay_ms: 10_000,
    });
```

Set `max_attempts: 1` to disable retries entirely.

---

## Error handling

All public async methods return `anyhow::Result<T>`.  Downcast `RateLimitedError` to check for 429:

```rust
use gflights::RateLimitedError;

match client.request_flights(&config).await {
    Ok(resp) => { /* use resp */ }
    Err(e) if e.downcast_ref::<RateLimitedError>().is_some() => {
        eprintln!("Rate limited — back off and retry");
    }
    Err(e) => eprintln!("Other error: {e}"),
}
```

---

## Known limitations

`x-goog-batchexecute-bgr` header — computed deep in Google's obfuscated JS from the current time and request payload length — is omitted.  Responses are still valid but may occasionally be less accurate (e.g. missing low-fare calendar data).  Contributions to reverse-engineer the algorithm are welcome.

---

## Development

```sh
# Build
cargo build

# Unit tests (no network)
cargo test

# Live integration tests (requires internet)
cargo test --test live_api -- --ignored

# Run examples
cargo run --example flights
cargo run --example graph

# Docs
cargo doc --open

# Lint
cargo clippy
```
