"""Live integration tests — require network access.

Run with:  RUN_LIVE_TESTS=1 pytest tests/test_live.py -v
"""

import asyncio
import os

import pytest
import gflights

live = pytest.mark.skipif(
    not os.environ.get("RUN_LIVE_TESTS"),
    reason="set RUN_LIVE_TESTS=1 to run live tests",
)


@pytest.fixture(scope="module")
def client():
    return gflights.GFlights()


@live
async def test_search_returns_flights(client):
    flights = await client.search(from_airport="LHR", to_airport="JFK", date="2026-09-01")
    assert isinstance(flights, list)
    assert len(flights) > 0


@live
async def test_search_flight_result_types(client):
    flights = await client.search(from_airport="MXP", to_airport="CDG", date="2026-09-15")
    assert len(flights) > 0
    f = flights[0]
    assert isinstance(f.airline, str) and len(f.airline) > 0
    assert isinstance(f.duration_minutes, int) and f.duration_minutes > 0
    assert isinstance(f.stops, int) and f.stops >= 0
    assert f.price is None or isinstance(f.price, int)
    assert isinstance(f.booking_token, str)


@live
async def test_search_legs_populated(client):
    flights = await client.search(from_airport="MXP", to_airport="LHR", date="2026-09-10")
    assert len(flights) > 0
    f = flights[0]
    legs = f.legs
    assert isinstance(legs, list) and len(legs) > 0
    leg = legs[0]
    assert isinstance(leg.from_airport, str)
    assert isinstance(leg.to_airport, str)
    assert isinstance(leg.departure_time, str)
    assert isinstance(leg.arrival_time, str)


@live
async def test_search_round_trip(client):
    flights = await client.search(
        from_airport="LHR", to_airport="JFK",
        date="2026-09-01", return_date="2026-09-15",
    )
    assert len(flights) > 0


@live
async def test_search_nonstop_filter(client):
    flights = await client.search(
        from_airport="LHR", to_airport="JFK", date="2026-09-01", stops="nonstop",
    )
    for f in flights:
        assert f.stops == 0, f"expected nonstop, got {f.stops} stops for {f.airline}"


@live
async def test_search_one_stop_filter(client):
    flights = await client.search(
        from_airport="LUX", to_airport="NRT", date="2026-09-01", stops="one-stop",
    )
    for f in flights:
        assert f.stops <= 1


@live
async def test_search_currency_us_dollar(client):
    flights = await client.search(
        from_airport="JFK", to_airport="LAX", date="2026-09-01", currency="us-dollar",
    )
    assert len(flights) > 0
    assert any(f.price is not None for f in flights)


@live
async def test_search_airline_include_filter(client):
    # Google returns codeshare partners alongside the filtered airline.
    flights = await client.search(
        from_airport="LHR", to_airport="JFK", date="2026-09-01", airlines_include=["BA"],
    )
    assert len(flights) > 0
    airlines = {f.airline for f in flights}
    assert airlines & {"BA", "AA", "IB", "multi"}, f"no BA-related airline in {airlines}"


@live
async def test_search_airline_exclude_filter(client):
    flights_all = await client.search(from_airport="CDG", to_airport="JFK", date="2026-09-01")
    flights_no_af = await client.search(
        from_airport="CDG", to_airport="JFK", date="2026-09-01", airlines_exclude=["AF"],
    )
    assert len(flights_no_af) > 0
    assert len(flights_no_af) <= len(flights_all)


@live
async def test_price_graph_returns_entries(client):
    entries = await client.price_graph(
        from_airport="LHR", to_airport="JFK", date="2026-09-01", months=2,
    )
    assert isinstance(entries, list) and len(entries) > 0
    e = entries[0]
    assert isinstance(e.date, str) and len(e.date) == 10
    assert isinstance(e.price, int) and e.price > 0


@live
async def test_price_graph_dates_are_sorted(client):
    entries = await client.price_graph(
        from_airport="MXP", to_airport="NRT", date="2026-09-01", months=1,
    )
    dates = [e.date for e in entries]
    assert dates == sorted(dates), "price graph entries not sorted by date"


@live
async def test_date_grid_returns_entries(client):
    entries = await client.date_grid(
        from_airport="LHR", to_airport="JFK",
        dep_start="2026-09-01", dep_end="2026-09-03",
        ret_start="2026-09-15", ret_end="2026-09-17",
    )
    assert isinstance(entries, list) and len(entries) > 0
    e = entries[0]
    assert isinstance(e.dep_date, str) and len(e.dep_date) == 10
    assert isinstance(e.ret_date, str) and len(e.ret_date) == 10
    assert isinstance(e.price, int) and e.price > 0


@live
async def test_date_grid_all_dates_in_range(client):
    entries = await client.date_grid(
        from_airport="MXP", to_airport="LHR",
        dep_start="2026-09-01", dep_end="2026-09-02",
        ret_start="2026-09-10", ret_end="2026-09-11",
    )
    for e in entries:
        assert "2026-09-01" <= e.dep_date <= "2026-09-02"
        assert "2026-09-10" <= e.ret_date <= "2026-09-11"


@live
async def test_concurrent_searches(client):
    """asyncio.gather runs both searches concurrently — no threads needed."""
    r1, r2 = await asyncio.gather(
        client.search(from_airport="LHR", to_airport="JFK", date="2026-09-01"),
        client.search(from_airport="MAD", to_airport="MEX", date="2026-09-01"),
    )
    assert len(r1) > 0 and len(r2) > 0


@live
async def test_search_lower_emissions_filter(client):
    flights = await client.search(
        from_airport="LHR", to_airport="AMS", date="2026-09-01", lower_emissions=True,
    )
    assert isinstance(flights, list)
