"""Smoke tests: import, instantiation, and exported names."""

import inspect

import gflights


def test_module_exports_expected_classes():
    expected = {
        "GFlights", "FlightResult", "LegInfo", "LayoverInfo",
        "EmissionsInfo", "PriceEntry", "DateGridEntry",
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
