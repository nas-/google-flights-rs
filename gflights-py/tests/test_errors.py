"""Tests that bad inputs raise the right Python exceptions (no Rust panic)."""

import pytest
import gflights


@pytest.fixture(scope="module")
def client():
    return gflights.GFlights()


def test_bad_date_format_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.search(from_airport="LHR", to_airport="JFK", date="01-08-2026")


def test_bad_return_date_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            return_date="not-a-date",
        )


def test_bad_currency_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown currency"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            currency="moon-coins",
        )


def test_bad_stops_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown stop option"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            stops="seventeen",
        )


def test_bad_travel_class_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown travel class"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            travel_class="platinum",
        )


def test_bad_sort_raises_value_error(client):
    with pytest.raises(ValueError, match="unknown sort order"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            sort="random",
        )


def test_bad_airline_filter_raises_value_error(client):
    with pytest.raises(ValueError, match="invalid airline filter"):
        client.search(
            from_airport="LHR",
            to_airport="JFK",
            date="2026-08-01",
            airlines_include=["INVALID_ALLIANCE_XYZ"],
        )


def test_date_grid_bad_dep_start_raises(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.date_grid(
            from_airport="LHR",
            to_airport="JFK",
            dep_start="bad",
            dep_end="2026-08-07",
            ret_start="2026-08-15",
            ret_end="2026-08-22",
        )


def test_price_graph_bad_date_raises(client):
    with pytest.raises(ValueError, match="invalid date"):
        client.price_graph(from_airport="LHR", to_airport="JFK", date="2026/08/01")
