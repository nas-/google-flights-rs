"""Unofficial async Python client for Google Flights, powered by Rust/tokio.

All search methods are coroutines — use with ``await`` or ``asyncio.gather``.

Quick start::

    import asyncio
    from gflights import Client

    async def main():
        client = Client()

        # Single search
        flights = await client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01")
        for f in flights:
            print(f.airline, f"{f.duration_minutes // 60}h{f.duration_minutes % 60}m", f.price)

        # Concurrent searches — no extra threads needed
        lhr_jfk, mad_mex = await asyncio.gather(
            client.search(from_airport="LHR", to_airport="JFK", date="2026-09-01"),
            client.search(from_airport="MAD", to_airport="MEX", date="2026-09-01"),
        )

    asyncio.run(main())
"""

from __future__ import annotations

from gflights._client import Client  # noqa: F401
from gflights._gflights import (  # noqa: F401
    BookingOption,
    CheapDate,
    DateGridEntry,
    DealResult,
    EmissionsInfo,
    ExploreResult,
    FlightResult,
    GFlightsError,
    LayoverInfo,
    LegInfo,
    Offer,
    PriceEntry,
)
from gflights._types import (  # noqa: F401
    Currency,
    Duration,
    SortOrder,
    StopFilter,
    TravelClass,
)

__all__ = [
    "Client",
    "GFlightsError",
    "FlightResult",
    "LegInfo",
    "LayoverInfo",
    "EmissionsInfo",
    "PriceEntry",
    "DateGridEntry",
    "CheapDate",
    "ExploreResult",
    "DealResult",
    "Offer",
    "BookingOption",
    "Currency",
    "Duration",
    "TravelClass",
    "StopFilter",
    "SortOrder",
]
