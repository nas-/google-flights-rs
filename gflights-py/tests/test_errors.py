"""Tests that bad inputs raise the right Python exceptions (no Rust panic).

Validation happens synchronously before the coroutine runs, so errors are
raised immediately without needing to await.
"""

import pytest
import gflights


@pytest.fixture(scope="module")
def client():
    return gflights.GFlights()


# All validation errors fire synchronously (before the future is awaited),
# so these tests work both inside and outside an event loop.

async def test_bad_date_format_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.search(from_airport="LHR", to_airport="JFK", date="01-08-2026")


async def test_bad_return_date_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.search(
            from_airport="LHR", to_airport="JFK",
            date="2026-08-01", return_date="not-a-date",
        )


def test_bad_currency_raises_value_error():
    # Currency is a client property; a bad code is rejected at construction.
    with pytest.raises(ValueError, match="unknown currency"):
        gflights.GFlights(currency="moon-coins")


async def test_bad_stops_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown stop option"):
        client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01",
                      stops="seventeen")


async def test_bad_travel_class_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown travel class"):
        client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01",
                      travel_class="platinum")


async def test_bad_sort_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown sort order"):
        client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01",
                      sort="random")


async def test_bad_airline_filter_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid airline filter"):
        client.search(from_airport="LHR", to_airport="JFK", date="2026-08-01",
                      airlines_include=["INVALID_ALLIANCE_XYZ"])


async def test_date_grid_bad_dep_start_raises(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.date_grid(
            from_airport="LHR", to_airport="JFK",
            dep_start="bad", dep_end="2026-08-07",
            ret_start="2026-08-15", ret_end="2026-08-22",
        )


async def test_price_graph_bad_date_raises(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.price_graph(from_airport="LHR", to_airport="JFK", date="2026/08/01")


# ---------------------------------------------------------------------------
# GFlightsError — typed exception class tests
# ---------------------------------------------------------------------------

def test_gflights_error_is_importable():
    """GFlightsError must be accessible at the top-level gflights package."""
    assert hasattr(gflights, "GFlightsError"), "gflights.GFlightsError not found"


def test_gflights_error_is_exception_subclass():
    """GFlightsError must be catchable as a plain Exception."""
    assert issubclass(gflights.GFlightsError, Exception)


def test_gflights_error_can_be_raised_and_caught():
    """GFlightsError can be raised and caught like any other exception."""
    with pytest.raises(gflights.GFlightsError, match="test"):
        raise gflights.GFlightsError("test")


def test_gflights_error_is_not_value_error():
    """GFlightsError is distinct from ValueError (not a subclass)."""
    assert not issubclass(gflights.GFlightsError, ValueError)


def test_gflights_error_is_not_runtime_error():
    """GFlightsError is distinct from RuntimeError (not a subclass)."""
    assert not issubclass(gflights.GFlightsError, RuntimeError)
