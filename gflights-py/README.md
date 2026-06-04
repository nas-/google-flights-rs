# gflights

Unofficial **async Python client for Google Flights**, powered by a Rust
backend (via [PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs)).

Search flights, price graphs, date grids, multi-city itineraries, cheapest
dates, and destination exploration — all as native `asyncio` coroutines, with
no browser or scraping framework required.

## Install

```sh
pip install gflights
```

Prebuilt wheels (CPython 3.10+, abi3) are published for Linux (x86_64),
Windows (x64), and macOS (arm64). Other platforms build from the source
distribution and require a Rust toolchain.

## Quick start

```python
import asyncio
from gflights import GFlights

async def main():
    client = GFlights()
    flights = await client.search(
        from_airport="LHR",
        to_airport="JFK",
        date="2026-09-15",
    )
    for f in flights[:5]:
        print(f.airline, f.price, f.duration_minutes, "min", f.stops, "stop(s)")

asyncio.run(main())
```

## Features

- `search` — one-way and round-trip flight search with filters (stops, airlines
  and alliances, connecting airports, max price, baggage, sort order, lower
  emissions).
- `price_graph` — cheapest fare per departure day across a date range.
- `date_grid` — full departure × return price matrix for round trips.
- `cheapest_dates` — cheapest departure dates over a range of months
  (one-way or fixed-length round trips).
- `multi_city_search` — open-jaw itineraries across multiple legs.
- `explore` — discover cheap destinations from an origin airport.
- Typed results (`FlightResult`, `CheapDate`, `ExploreResult`, `EmissionsInfo`,
  `LayoverInfo`, `LegInfo`) and full `.pyi` stubs for IDE/mypy support.
- Built-in rate limiting and retry with 429 detection.

## Proxy & User-Agent

```python
client = GFlights(
    proxy="socks5://127.0.0.1:9050",   # http://, https://, or socks5://
    user_agent="Mozilla/5.0 ...",      # default: rotating real desktop UA
)
```

## Error handling

All API calls raise `GFlightsError` on network or parse failures:

```python
from gflights import GFlights, GFlightsError

try:
    flights = await client.search(from_airport="LHR", to_airport="JFK", date="2026-09-15")
except GFlightsError as e:
    print("lookup failed:", e)
```

## Links & license

- Source, full documentation, and the Rust crate:
  <https://github.com/nas-/google-flights-rs>

MIT licensed. This is an unofficial client and is not affiliated with Google.
