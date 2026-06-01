# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`total_time_minutes`** field on `Itinerary` (index 9 of raw response).
- **`connection_info`** field on `Itinerary` — per-layover `ConnectionInfo` structs
  (connection time, arrival/departure airport codes and city names).
- **`stop_count()`** method on `Itinerary` — derived from `connection_info` length.
- **`leg_duration_minutes`** field on `FlightInfo` (individual leg duration).
- **`emissions`** field on `Itinerary` — CO2 data (`Emissions` struct with
  `co2_this_flight_g`, `co2_typical_route_g`, `co2_lowest_route_g`, and
  `emission_vs_average_percent`).
- **`language` / `country`** fields on `Config` and `ConfigBuilder` — removes
  hard-coded `hl=en-GB` locale; default remains `en`/`GB`.
- **`sort_order`** field on `Config` (`SortOrder` enum: Best, Price, Duration,
  DepartureTime, ArrivalTime).
- **`stopover_min`** field on `Config` — minimum layover duration complementing
  the existing `stopover_max`.
- **Retry logic** (`RetryConfig`) — exponential back-off (base × 2ⁿ, capped)
  with jitter for 5xx and timeout errors; configurable via
  `ApiClient::with_retry_config()`.
- **`gflights` CLI binary** — `search`, `graph`, and `offer` subcommands with
  `--format table|json` output and full `CommonArgs` (airports, dates, class,
  stops, currency, locale).
- **Sync builder setters** `departure_location(Location)` and
  `destination_location(Location)` on `ConfigBuilder` (for unit tests that
  cannot call the async city-lookup methods).
- **Live integration tests** (`tests/live_api.rs`, gated behind
  `RUN_LIVE_TESTS=1`): locale test (fr-FR), concurrency test (3 parallel
  tasks), click-token test, invalid-IATA test, shared `OnceCell<ApiClient>`.
- **Unit tests** — 112 tests total (was 54); parser line coverage ≥ 80 %.
- **CI** (`.github/workflows/ci.yml`) — build, unit+doc tests, Clippy, Rustfmt,
  and Rustdoc on Ubuntu and Windows.
- **`CHANGELOG.md`** and improved `README.md` with features table, quick-start
  examples, config reference, retry / rate-limiting docs.

### Changed

- `FlightRequestOptions`, `GraphRequestOptions`, and `ItineraryRequest::new()`
  now accept `stopover_min: &StopoverDuration`.
- `SingleLegStruct::serialize_to_web()` format string extended to position 13
  for `stopover_min` (inferred; verify with live traffic if needed).
- `do_request()` now retries on 5xx and timeout rather than propagating the
  first failure.
- `tracing-subscriber` moved from `[dev-dependencies]` to `[dependencies]`
  (required by the `gflights` binary at runtime).

### Fixed

- `Travelers::new([0, …])` now returns `Err` instead of silently constructing
  an invalid `Travelers` with 0 adults.
- `PlaceType::from(i32)` with unknown discriminants now logs a warning and
  returns `Unspecified` instead of panicking.

[Unreleased]: https://github.com/YOUR_USERNAME/google-flights-rs/compare/HEAD...HEAD
