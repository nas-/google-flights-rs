"""Smoke tests: import, instantiation, and exported names."""

import inspect

import pytest

import gflights


def test_module_exports_expected_classes():
    expected = {
        "GFlights", "FlightResult", "LegInfo", "LayoverInfo",
        "EmissionsInfo", "PriceEntry", "DateGridEntry", "CheapDate",
    }
    assert expected.issubset(set(dir(gflights)))


def test_type_aliases_exported():
    from gflights import TravelClass, StopFilter, SortOrder
    # They are typing.Literal objects at runtime
    assert TravelClass is not None
    assert StopFilter is not None
    assert SortOrder is not None


def test_gflights_instantiates():
    client = gflights.GFlights()
    assert repr(client) == "GFlights()"


def test_gflights_not_rate_limited_by_default():
    client = gflights.GFlights()
    assert client.rate_limited is False


def test_reset_rate_limit_does_not_raise():
    client = gflights.GFlights()
    client.reset_rate_limit()


async def test_search_returns_awaitable():
    """Methods return awaitables (asyncio.Future) when called inside an event loop."""
    client = gflights.GFlights()
    fut = client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01")
    assert inspect.isawaitable(fut)
    fut.cancel()  # don't actually run the network call


async def test_price_graph_returns_awaitable():
    client = gflights.GFlights()
    fut = client.price_graph(from_airport="LHR", to_airport="JFK", date="2026-08-01")
    assert inspect.isawaitable(fut)
    fut.cancel()


async def test_date_grid_returns_awaitable():
    client = gflights.GFlights()
    fut = client.date_grid(
        from_airport="LHR", to_airport="JFK",
        dep_start="2026-08-01", dep_end="2026-08-03",
        ret_start="2026-08-15", ret_end="2026-08-17",
    )
    assert inspect.isawaitable(fut)
    fut.cancel()


async def test_multi_city_search_returns_awaitable():
    """multi_city_search returns an awaitable when called inside an event loop."""
    client = gflights.GFlights()
    fut = client.multi_city_search([
        ("LHR", "JFK", "2026-08-01"),
        ("JFK", "LHR", "2026-08-15"),
    ])
    assert inspect.isawaitable(fut)
    fut.cancel()


async def test_multi_city_search_raises_for_single_leg():
    """multi_city_search raises ValueError synchronously for 1 leg."""
    client = gflights.GFlights()
    with pytest.raises(ValueError, match="2 legs"):
        client.multi_city_search([("LHR", "JFK", "2026-08-01")])


async def test_cheapest_dates_returns_awaitable():
    client = gflights.GFlights()
    fut = client.cheapest_dates(from_airport="LHR", to_airport="JFK", date="2026-08-01")
    assert inspect.isawaitable(fut)
    fut.cancel()


async def test_cheapest_dates_round_trip_returns_awaitable():
    client = gflights.GFlights()
    fut = client.cheapest_dates(
        from_airport="LHR", to_airport="JFK",
        date="2026-08-01", months=3, trip_duration_days=7,
    )
    assert inspect.isawaitable(fut)
    fut.cancel()


def test_cheap_date_class_exported():
    assert hasattr(gflights, "CheapDate")
