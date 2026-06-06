# gflights

[![crates.io](https://img.shields.io/crates/v/gflights)](https://crates.io/crates/gflights)
[![docs.rs](https://img.shields.io/docsrs/gflights)](https://docs.rs/gflights)
[![CI](https://github.com/nas-/google-flights-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/nas-/google-flights-rs/actions/workflows/ci.yml)

Unofficial async Rust client for the [Google Flights](https://www.google.com/flights) web API.

Search flights, compare prices across a date range, retrieve booking offers, and resolve booking URLs — all without an official API key.

> **Disclaimer:** This library talks to the same endpoints used by the Google Flights website.  It is not affiliated with or endorsed by Google.  Usage is subject to Google's Terms of Service.

---

## Features

- **Flight search** — one-way, return, multi-stop itineraries
- **Price graph** — cheapest fares across a configurable date range
- **Date grid** — full departure × return price matrix for round trips
- **Booking offers** — airline/OTA offers with prices and booking URLs
- **Flight deals** — discounted destinations from an origin (price vs typical, discount %, booking link)
- **City / airport lookup** — resolve city names and IATA codes
- **Multi-airport search** — up to 7 departure or destination airports
- **Airline / alliance filters** — include or exclude specific airlines or alliances (oneworld, SkyTeam, Star Alliance)
- **Connection filters** — require layover through specific airports; set min/max layover duration
- **Lower-emissions filter** — restrict to flights with below-average CO₂
- **Locale support** — `language` + `country` for non-English results
- **Sort order** — Best · Price · Duration · Departure time · Arrival time
- **CO2 / emissions** — included in parsed itinerary data
- **Layover details** — connection time, airport codes, overnight warnings
- **Rate limiting** — built-in governor-based token-bucket limiter
- **Retry logic** — exponential back-off for transient 5xx / timeout errors
- **CLI** — interactive REPL and one-shot subcommands (`search`, `graph`, `dgrid`, `offer`)

---

## Installation

```toml
[dependencies]
gflights = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

---

## CLI

The crate ships a `gflights` binary. Install it with:

```sh
cargo install gflights
```

### One-shot mode

```sh
# Search flights
gflights search --from LHR --to JFK --date 2026-08-01

# Round trip with filters
gflights search --from MXP --to NRT --date 2026-09-01 --return 2026-09-15 \
  --airline LX --airline ONEWORLD --via ZRH \
  --min-layover 60 --max-layover 180 \
  --lower-emissions --sort price --format json

# Multi-city (open-jaw) search
gflights mcity --leg LUX FCO 2026-09-10 --leg FCO MAD 2026-09-13 --leg MAD LUX 2026-09-17

# Price graph (cheapest fare per day over 3 months)
gflights graph --from LHR --to JFK --date 2026-08-01 --months 3

# Departure × return price grid
gflights dgrid --from LHR --to JFK \
  --dep-start 2026-08-01 --dep-end 2026-08-07 \
  --ret-start 2026-08-15 --ret-end 2026-08-22

# Booking offers with clickable URLs (OSC 8, supported in most modern terminals)
gflights offer --from FRA --to SIN --date 2026-10-01

# Find discounted destinations (Google Flights deals)
gflights deals --from LUX --out 2026-06-20 --ret 2026-06-24 --nonstop

# Explore cheap destinations (Google Flights Explore)
gflights explore --from LUX --month 9 --duration week --budget 150 --interest climbing

# Find cheapest departure dates (one-way)
gflights cheap --from LHR --to BCN --date 2026-08-01 --months 3

# Find cheapest round-trip combinations (fixed trip length)
gflights cheap --from LHR --to BCN --date 2026-08-01 --months 3 --trip-days 7

# Search with emissions column and layover detail
gflights search --from LUX --to SYD --date 2026-09-01 --show-co2 --detail
```

### Interactive REPL

Run `gflights` with no arguments to enter an interactive session with history:

```
gflights> search --from LHR --to JFK --date 2026-08-01
gflights> graph  --from MXP --to SYD --date 2026-09-01 --months 2
gflights> dgrid  --from LHR --to JFK --dep-start 2026-08-01 --dep-end 2026-08-07 --ret-start 2026-08-15 --ret-end 2026-08-22
gflights> quit
```

### `search` flag reference

| Flag | Default | Description |
|---|---|---|
| `--from <CODE>` | required | Departure airport IATA code or city name |
| `--to <CODE>` | required | Destination airport IATA code or city name |
| `--date <YYYY-MM-DD>` | required | Outbound departure date |
| `--return <YYYY-MM-DD>` | one-way | Return date |
| `--adults <N>` | `1` | Number of adult passengers |
| `--class <CLASS>` | `economy` | `economy` · `premium-economy` · `business` · `first` |
| `--stops <STOPS>` | `all` | `all` · `non-stop` · `one-stop` |
| `--sort <SORT>` | `best` | `best` · `price` · `duration` · `departure-time` · `arrival-time` ¹ |
| `--airline <CODE>` | — | Include airline IATA code or alliance (`ONEWORLD`, `SKYTEAM`, `STAR_ALLIANCE`). Repeatable. |
| `--exclude-airline <CODE>` | — | Exclude airline or alliance. Repeatable. |
| `--via <IATA>` | — | Require connection through this airport. Repeatable. |
| `--min-layover <MINS>` | none | Minimum layover in minutes (rounded up to 30 min intervals) |
| `--max-layover <MINS>` | none | Maximum layover in minutes |
| `--lower-emissions` | off | Restrict to below-average CO₂ flights |
| `--show-co2` | off | Add a CO₂ kg column to the table output |
| `--detail` | off | Show layover airports (`via ZRH (65 min)`) and `+1` for next-day arrivals |
| `--currency <CURRENCY>` | `euro` | Result currency (e.g. `us-dollar`, `british-pound`) |
| `--lang <CODE>` | `en` | BCP-47 language subtag |
| `--country <CODE>` | `GB` | ISO 3166-1 alpha-2 country code |
| `--format <FORMAT>` | `table` | `table` · `json` |

¹ `departure-time` and `arrival-time` are sorted client-side after Google returns results.

### `dgrid` flag reference

| Flag | Default | Description |
|---|---|---|
| `--from <CODE>` | required | Departure airport IATA code or city name |
| `--to <CODE>` | required | Destination airport IATA code or city name |
| `--dep-start <DATE>` | required | First outbound departure date |
| `--dep-end <DATE>` | required | Last outbound departure date |
| `--ret-start <DATE>` | required | First return date |
| `--ret-end <DATE>` | required | Last return date |
| `--adults <N>` | `1` | Number of adult passengers |
| `--class <CLASS>` | `economy` | Travel class |
| `--stops <STOPS>` | `all` | Stop filter |
| `--currency <CURRENCY>` | `euro` | Result currency |
| `--format <FORMAT>` | `table` | `table` · `json` |

### Global flags (any subcommand)

| Flag | Default | Description |
|---|---|---|
| `--proxy <URL>` | none | Route all requests through a proxy. Supports `http://`, `https://`, `socks5://` (e.g. `socks5://127.0.0.1:9050`). |
| `--user-agent <UA>` | random | Override the User-Agent. By default a real desktop browser string is chosen from a rotating pool per run. |

```sh
# Search through a local SOCKS5 proxy
gflights --proxy socks5://127.0.0.1:9050 search --from LHR --to JFK --date 2026-09-15
```

---

## Proxy & Docker sidecar

Every request — including the one-time frontend-version probe — is routed
through the configured proxy:

```rust
let client = ApiClient::new_with_proxy("socks5://127.0.0.1:9050").await?;
```

```python
from gflights import Client
client = Client(proxy="socks5://127.0.0.1:9050")
```

For rotating egress IPs, run a proxy as a sidecar container and share its
network namespace. The included [`docker-compose.yml`](docker-compose.yml) shows
the pattern — `gflights` runs with `network_mode: "service:proxy"`, so all of
its traffic leaves through the `proxy` container:

```sh
docker compose run --rm gflights search --from LHR --to JFK --date 2026-09-15
```

Swap the `proxy` service for any HTTP/SOCKS5 proxy (or a rotating-IP service)
without touching the `gflights` service.

---

## MCP server

Run gflights as a [Model Context Protocol](https://modelcontextprotocol.io)
server over stdio, exposing flight tools to MCP clients such as Claude Desktop:

```sh
gflights mcp
```

It speaks JSON-RPC 2.0 on stdin/stdout and exposes these tools: `search`,
`price_graph`, `cheapest_dates`, `explore`, `deals`. Each maps its JSON arguments
to the same library calls the CLI uses and returns the result as JSON.

Example client configuration (Claude Desktop `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "gflights": {
      "command": "gflights",
      "args": ["mcp"]
    }
  }
}
```

The global `--proxy` and `--user-agent` flags work here too, e.g.
`"args": ["--proxy", "socks5://127.0.0.1:9050", "mcp"]`.

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

### Multi-city (open-jaw) search

```rust
use gflights::requests::{api::ApiClient, config::MultiCityConfig};
use chrono::NaiveDate;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ApiClient::new().await;

    let config = MultiCityConfig::builder()
        .add_leg("LUX", "FCO", NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(), &client).await?
        .add_leg("FCO", "MAD", NaiveDate::from_ymd_opt(2026, 9, 13).unwrap(), &client).await?
        .add_leg("MAD", "LUX", NaiveDate::from_ymd_opt(2026, 9, 17).unwrap(), &client).await?
        .build()?;

    let results = client.request_multi_city_flights(&config).await?;
    let flights = results.get_all_flights();
    println!("Found {} flight options across {} legs", flights.len(), config.legs.len());
    Ok(())
}
```

Run with:

```sh
cargo run --example multi_city
```

---

## Python bindings

The `gflights-py/` directory provides async Python bindings built with [pyo3](https://pyo3.rs) and [maturin](https://www.maturin.rs).

### Install

```sh
pip install gflights          # when published to PyPI
# or build from source:
cd gflights-py && maturin develop
```

### Quick start

Route arguments are `origin` / `destination` (each takes an IATA code **or** a
city name). Passenger counts are grouped into a `Passengers` object and the
shared result filters into a `SearchFilters` object — pass only what you need.

```python
import asyncio
from gflights import Client, Passengers, SearchFilters

async def main():
    client = Client()

    # One-way search
    flights = await client.search(
        origin="LHR", destination="JFK", date="2026-08-01",
    )
    for f in flights:
        print(f.airline, f.duration_minutes, f.price)

    # Two adults + a child, non-stop only, sorted by price
    flights = await client.search(
        origin="LHR", destination="JFK", date="2026-08-01",
        passengers=Passengers(adults=2, children=1),
        filters=SearchFilters(stops="nonstop", sort="price"),
    )

    # Price graph — cheapest fare per day over 3 months
    graph = await client.price_graph(
        origin="LHR", destination="JFK", date="2026-08-01", months=3
    )
    cheapest = min(graph, key=lambda e: e.price)
    print(cheapest.date, cheapest.price)

    # Departure × return price grid
    grid = await client.date_grid(
        origin="LHR", destination="JFK",
        dep_start="2026-08-01", dep_end="2026-08-07",
        ret_start="2026-08-14", ret_end="2026-08-21",
    )
    best = min(grid, key=lambda e: e.price)
    print(best.dep_date, "→", best.ret_date, best.price)

    # Cheapest departure dates (one-way)
    dates = await client.cheapest_dates(
        origin="LHR", destination="JFK", date="2026-08-01", months=3
    )
    for d in dates[:5]:
        print(d.departure_date, d.price)

    # Cheapest round-trip combinations (7-night stay)
    rt_dates = await client.cheapest_dates(
        origin="LHR", destination="JFK",
        date="2026-08-01", months=3, trip_duration_days=7
    )
    for d in rt_dates[:5]:
        print(d.departure_date, "→", d.return_date, d.price)

    # Explore cheap destinations
    dests = await client.explore(
        origin="LUX", month=9, duration="week",
        max_price=200, interest="beaches",
    )
    for d in sorted(dests, key=lambda x: x.price or 9999)[:5]:
        print(d.name, d.country, d.flight_airport or d.nearest_airport, d.price)

    # Run multiple searches concurrently
    lhr_jfk, mad_mex = await asyncio.gather(
        client.search(origin="LHR", destination="JFK", date="2026-09-01"),
        client.search(origin="MAD", destination="MEX", date="2026-09-01"),
    )

asyncio.run(main())
```

### Error handling

All API errors raise `gflights.GFlightsError` (a subclass of `Exception`).
Input validation errors (bad date, unknown currency, etc.) raise `ValueError`.

```python
try:
    flights = await client.search(origin="LHR", destination="JFK", date="2026-08-01")
except gflights.GFlightsError as e:
    print(f"API error: {e}")
except ValueError as e:
    print(f"Bad input: {e}")
```

### Rate limiting

The `client.rate_limited` flag is set to `True` when Google returns HTTP 429.
Call `client.reset_rate_limit()` after a cooling-off period.

### Python type stubs

Full `.pyi` stubs are shipped with the package.  Every method and class is typed and documented, so IDE auto-completion and mypy work out of the box.

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
| `.stopover_min(d)` | `StopoverDuration` | Unlimited | Minimum layover duration |
| `.stopover_max(d)` | `StopoverDuration` | Unlimited | Maximum layover duration |
| `.duration_max(d)` | `TotalDuration` | Unlimited | Maximum total trip duration |
| `.departing_times(t)` | `FlightTimes` | Any | Outbound departure time window |
| `.return_times(t)` | `FlightTimes` | Any | Return departure time window |
| `.airlines_include(v)` | `Vec<AirlineFilter>` | none | Restrict to these airlines / alliances |
| `.add_airline_include(f)` | `AirlineFilter` | — | Add one airline / alliance to include filter |
| `.airlines_exclude(v)` | `Vec<AirlineFilter>` | none | Exclude these airlines / alliances |
| `.add_airline_exclude(f)` | `AirlineFilter` | — | Add one airline / alliance to exclude filter |
| `.connecting_airports(v)` | `Vec<String>` | none | Require connection through these IATA airport codes |
| `.add_connecting_airport(s)` | `&str` | — | Add one connecting airport |
| `.lower_emissions(b)` | `bool` | `false` | Restrict to below-average CO₂ flights |

### Travelers

```rust
use gflights::parsers::common::Travelers;

// [adults, children, infants_in_seat, infants_on_lap]
let travelers = Travelers::new(vec![2, 1, 0, 0])?; // 2 adults + 1 child
```

Rules: at least 1 adult, total ≤ 9 passengers.

### Airline & connection filters

```rust
use gflights::parsers::common::{AirlineFilter, Alliance};

let config = Config::builder()
    .departure("LHR", &client).await?
    .destination("JFK", &client).await?
    .departing_date(date)
    // Only show British Airways and oneworld alliance members
    .add_airline_include("BA".parse::<AirlineFilter>()?)
    .add_airline_include(AirlineFilter::Alliance(Alliance::OneWorld))
    // Must connect through Dublin
    .add_connecting_airport("DUB")
    // At least 45 min layover, at most 3 hours
    .stopover_min(StopoverDuration::Minutes(45))
    .stopover_max(StopoverDuration::Minutes(180))
    // Lower CO₂ only
    .lower_emissions(true)
    .build()?;
```

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
cargo test --lib

# Binary (CLI) tests
cargo test --bin gflights

# Doc tests
cargo test --doc

# Live integration tests (requires internet, skipped in CI)
RUN_LIVE_TESTS=1 cargo test --lib -- --ignored

# Docs
cargo doc --open

# Lint
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt

# Security audit
cargo audit
```

---

## Contributing

Install the pre-commit hook once per clone:
```sh
git config core.hooksPath hooks
chmod +x hooks/pre-commit  # Unix/macOS only
```
The hook runs `cargo fmt`, `cargo clippy`, `rustdoc`, `cargo test --lib`, and offline pytest
before every commit. Requires `maturin develop` to have been run at least once for the Python check.
