"""Unofficial async Python client for Google Flights, powered by Rust/tokio.

All search methods are coroutines — use with ``await`` or ``asyncio.gather``.

Quick start::

    import asyncio
    import gflights

    async def main():
        client = gflights.GFlights()

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

from gflights._gflights import (  # noqa: F401
    CheapDate,
    DateGridEntry,
    EmissionsInfo,
    ExploreResult,
    FlightResult,
    GFlights,
    LayoverInfo,
    LegInfo,
    PriceEntry,
)
from gflights._types import SortOrder, StopFilter, TravelClass  # noqa: F401

__all__ = [
    "GFlights",
    "FlightResult",
    "LegInfo",
    "LayoverInfo",
    "EmissionsInfo",
    "PriceEntry",
    "DateGridEntry",
    "CheapDate",
<<<<<<< HEAD
    "ExploreResult",
=======
>>>>>>> feat/cheapest-dates
    "TravelClass",
    "StopFilter",
    "SortOrder",
]
