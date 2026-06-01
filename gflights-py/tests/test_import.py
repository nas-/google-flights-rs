"""Smoke tests: import, instantiation, and exported names."""

import gflights


def test_module_exports_expected_classes():
    expected = {
        "GFlights",
        "FlightResult",
        "LegInfo",
        "LayoverInfo",
        "EmissionsInfo",
        "PriceEntry",
        "DateGridEntry",
    }
    assert expected.issubset(set(dir(gflights)))


def test_gflights_instantiates():
    client = gflights.GFlights()
    assert repr(client) == "GFlights()"


def test_gflights_not_rate_limited_by_default():
    client = gflights.GFlights()
    assert client.rate_limited is False


def test_reset_rate_limit_does_not_raise():
    client = gflights.GFlights()
    client.reset_rate_limit()  # should be a no-op, not raise
