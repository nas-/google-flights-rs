# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] — 2026-06-06

### Added

- **Flight deals** (`GetFlightDealsStreaming`) — `ApiClient::request_deals` /
  `DealConfig` / `DealResult` return discounted destinations from an origin with
  price vs typical price, discount %, airline, stops, duration, dates, and a
  ready-to-open booking deep link. Exposed via the `deals` CLI subcommand, the
  Python `Client.deals(...)` method, an MCP `deals` tool, and `examples/deals.rs`.
  Supports `--nonstop` and `--max-hours` filters.
- **Python `offer()`** — price the cheapest itinerary and return booking offers
  with resolved booking URLs. New `Offer` / `BookingOption` result classes.
- **Children & infant passengers** — `children`, `infants_in_seat` and
  `infants_on_lap` on every passenger endpoint (CLI `--children`,
  `--infants-seat`, `--infants-lap`; on the Python client via `Passengers`).
- **Full filters on price/date endpoints** — `price_graph`, `date_grid` and
  `cheapest_dates` now accept the same filters as `search` (class, stops,
  airlines, via, max price, baggage, lower emissions, passengers).
- **Python `Passengers` and `SearchFilters` dataclasses** — group the
  per-traveller counts and the shared result filters into two objects instead
  of passing a dozen keyword arguments to every method. Both are exported from
  `gflights` and carry sensible defaults.
- **`Currency` enum** and `datetime.date` inputs in the Python client; date
  arguments accept `"YYYY-MM-DD"` strings or `datetime.date` objects.
- **Rotating User-Agent pool** — each `ApiClient` now selects a real desktop
  browser User-Agent from a pool at construction instead of sending one fixed
  string, reducing trivial fingerprinting. Override with
  `ApiClient::with_user_agent(...)`, the CLI `--user-agent` flag, or the Python
  `Client(user_agent=...)` argument. New `ApiClient::user_agent()` getter.
- **Proxy support** — route every request (including the frontend-version
  probe) through an `http://`, `https://`, or `socks5://` proxy via
  `ApiClient::new_with_proxy(...)`, the CLI `--proxy` flag, or the Python
  `Client(proxy=...)` argument. Added a `Dockerfile` and `docker-compose.yml`
  demonstrating a proxy-sidecar deployment with a shared network namespace.
- **`select` subcommand** — interactive booking flow: pick an outbound flight
  (and a return for round trips) by number, then a booking offer, and get the
  resolved booking URL. Works one-shot or inside the REPL.
- **`mcp` subcommand** — run as a Model Context Protocol server over stdio
  (JSON-RPC 2.0), exposing `search`, `price_graph`, `cheapest_dates`,
  `explore`, and `deals` tools to MCP clients such as Claude Desktop. Honours
  the global `--proxy` and `--user-agent` flags.

### Changed

- **Python: `GFlights` → `Client` (breaking).** The public client is now
  `gflights.Client`, a pure-Python wrapper over the Rust engine with explicitly
  typed signatures, full docstrings, and input normalization. No back-compat
  alias.
- **Python: locale is a client property (breaking).** `currency` / `lang` /
  `country` move from per-call arguments to the `Client(...)` constructor.
  `currency` takes an ISO-4217 code (e.g. `"USD"`) or a `Currency` member —
  the old kebab `ValueEnum` names (`us-dollar`) are no longer accepted.
- **Python: route arguments renamed `from_airport`/`to_airport` →
  `origin`/`destination` (breaking).** Each still accepts an IATA code or a
  city name; the new names reflect that. Applies to every route method
  (`search`, `price_graph`, `date_grid`, `cheapest_dates`, `offer`) plus
  `explore`/`deals` (`origin`).
- **Python: passenger counts and result filters are grouped (breaking).** The
  route methods now take `passengers=Passengers(...)` and
  `filters=SearchFilters(...)` instead of individual `adults` / `stops` / `sort`
  / `via` / … keyword arguments.
- **Python `deals` signature aligned with the other methods (breaking).**
  `out`/`ret` → `date`/`return_date`, and the boolean `nonstop` → `stops`
  (a `StopFilter`; the deals endpoint only distinguishes non-stop from
  any-stops).
- Result classes gained `.to_dict()` and pythonic `__repr__` (no leaked Rust
  `Some(..)`).
- CLI: `--currency` / `--lang` / `--country` are now global flags instead of
  per-subcommand options.

### Fixed

- **Strict `via` (connecting-airport) filter.** Google's connecting-airport
  filter is a *soft* server-side hint — the `other_flights` container still
  returns non-stop itineraries that skip the requested airport. Results are now
  filtered client-side so a `via` search only returns itineraries that actually
  connect through one of the requested airports. New
  `Itinerary::connects_via` / `Itinerary::layover_airports` and
  `FlightResponseContainer::get_all_flights_via`; the wire encoding at leg
  position `[9]` is verified byte-identical to the Google web UI. Applied to
  CLI `search` and the Python `search` / `offer` paths.
- PyPI project page showed no description: the Python package now ships a
  `readme` (`gflights-py/README.md`) so the long description is rendered.
- `explore(interest=...)` now resolves interest names (e.g. `"beaches"`) and
  raises on unknown values instead of silently returning no results.

---

## [0.2.1] — 2026-06-04

### Changed

- TLS backend switched from native-tls (vendored OpenSSL) to **rustls**. This
  removes the OpenSSL — and its Perl — build dependency, making builds hermetic
  on every platform and fixing portable (manylinux) wheel builds. No API change.

---

## [0.2.0] — 2026-06-04

### Added

#### Core library

- **`gflights explore`** — `GetExploreDestinations` endpoint + CLI `explore` subcommand.
  Returns `Vec<ExploreResult>` with destination name, country, airport codes, price, dates,
  airline, CO₂, and accommodation price.
  - `ExploreResult::flight_airport` — actual flight destination airport (may differ from the
    geographic `nearest_airport`; e.g. Verdon Gorge shows MRS, not NCE).
  - `--interest` flag accepts names (`beaches`, `climbing`, …), aliases, or raw Knowledge-Graph
    MIDs (`/m/…`).
  - `--to` flag filters to a destination airport or geographic region (Alps, Northern Europe, …).
- **`request_cheapest_dates`** — scans a range of months for the cheapest one-way or round-trip
  departure dates; returns `Vec<CheapDate>` sorted by price.
- **`request_date_grid`** — full departure × return price matrix from `GetCalendarGrid`;
  returns `Vec<DateGridEntry>`.  Now runs chunks concurrently (`buffer_unordered(8)`)
  with per-chunk body-read retry on EOF, reducing round-trip scan time from ~10 min to ~30 s.
- **Multi-city (open-jaw) search** — `MultiCityConfig::builder()` with per-leg
  `max_price`, `carry_on`, `checked_bags` filters.
- **Up to 7 airports per side** — `departure` / `destination` (and each multi-city
  leg) now accept up to 7 origin/destination airports, matching Google's maximum
  (previously capped at 4).
- **Arrives-next-day detection** — `Itinerary::arrives_next_day()`, `arrival_date()`,
  `departure_date()` derived from raw date fields.
- **`max_price` filter** on `Config` and `MultiCityConfig`.
- **Baggage filter** (`carry_on`, `checked_bags`) on `Config`.
- **`AirlineFilter`** with `Alliance` variants (OneWorld, SkyTeam, StarAlliance) and
  `FromStr` parsing (`"LX"`, `"ONEWORLD"`, `"star_alliance"`).
- **`stopover_min`** field on `Config` — minimum layover duration.
- **`language` / `country`** on `Config` and `ExploreConfig` — removes hard-coded `en-GB`.
- **`sort_order`** field on `Config` (`SortOrder` enum: Best, Price, Duration,
  DepartureTime, ArrivalTime).
- **Rate-limiter** (governor token bucket, 10 req/s default) with 429 detection flag and
  `reset_rate_limit()`.  Shared across all clones of an `ApiClient`.
- **Retry logic** (`RetryConfig`) — exponential back-off for 5xx/timeout; configurable via
  `ApiClient::with_retry_config()`.  `request_date_grid` also retries body-read errors.
- **`RateLimitedError`** — downcasting sentinel for HTTP 429.
- **`Travelers::new()`** — validates ≥1 adult, total ≤9; previously unchecked.
- **`PlaceType::from(i32)` non-panicking** — unknown discriminants return `Unspecified`
  with a `tracing::warn!` instead of panicking.

#### CLI (`gflights` binary)

- `search` subcommand: `--airline`, `--exclude-airline`, `--via`, `--min-layover`,
  `--max-layover`, `--lower-emissions`, `--sort`, `--format`.
- `search --show-co2` — CO₂ kg column in table output.
- `search --detail` — layover airports (`via ZRH (65 min)`) and `+1` next-day marker.
- `graph` subcommand.
- `dgrid` subcommand.
- `offer` subcommand (booking offers + deep links).
- `cheap` subcommand (cheapest departure dates, `--trip-days N` for round trips).
- `explore` subcommand with `--interest`, `--to`, `--duration`, `--month`, `--budget`.
- `mcity` (multi-city) subcommand.
- Interactive **REPL** with readline history; `--help` works inside the REPL.

#### Python bindings (`gflights-py`)

- Async Python bindings via pyo3 + maturin.
- Wheels use the CPython **abi3** stable ABI: a single wheel per platform works
  on CPython 3.10+.
- `GFlights` class with methods: `search`, `price_graph`, `date_grid`,
  `multi_city_search`, `explore`, `cheapest_dates`.
- **`GFlightsError`** — typed exception class; all API errors now raise
  `GFlightsError` (a `Exception` subclass) instead of `RuntimeError`.
- Full `.pyi` type stubs for IDE support and mypy compatibility.
- `CheapDate`, `ExploreResult`, `EmissionsInfo`, `LayoverInfo`, `LegInfo` data classes.
- `rate_limited` flag and `reset_rate_limit()` on `GFlights`.

#### Tests

- **Wire-protocol tests** (`tests/wire.rs`) — 19 tests feeding captured fixtures through
  parsers and asserting field values.
- **Negative / error-input tests** (`tests/negative.rs`) — 39 tests covering bad input
  validation, malformed response bodies, and error message content.
- Live integration tests (`tests/live_api.rs`, gated behind `RUN_LIVE_TESTS=1`).
- Python offline test suite: `test_import.py`, `test_types.py`, `test_errors.py`.

### Fixed

- `Travelers::new([0, …])` returned `Ok` with 0 adults; now returns `Err`.
- `PlaceType::from(i32)` panicked on undocumented values; now logs warning and
  returns `Unspecified`.
- `explore` CLI ARPT column showed geographic nearest airport (NCE) instead of the
  actual flight destination airport (MRS for Verdon Gorge).
- REPL `--help` flag triggered `UnknownArgument` error instead of printing help.
- `_gflights.pyi` had unresolved merge-conflict markers from `feat/multi-city`;
  `explore()`, `cheapest_dates()`, and `multi_city_search(max_price, …)` signatures
  were missing.
- Date-grid chunking looped without moving the window, producing all-identical requests.

### Changed

- `request_date_grid` chunks are now dispatched concurrently with `buffer_unordered(8)`.
- `do_request()` retries 5xx/timeout errors; `request_date_grid_chunk` additionally
  retries body-read errors (EOF from mid-stream connection closure).
- CI runs on pull requests only (not on every push to master).

---

## [0.1.0] — 2026-04-01

Initial release.

- Flight search (one-way and return).
- Price graph (`GetCalendarGraph`).
- Booking offer resolution.
- City / airport lookup.

[Unreleased]: https://github.com/nas-/google-flights-rs/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/nas-/google-flights-rs/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/nas-/google-flights-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/nas-/google-flights-rs/releases/tag/v0.1.0
