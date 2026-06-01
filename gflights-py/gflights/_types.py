"""Literal type aliases for every string-enum parameter in the gflights API."""

from typing import Literal

TravelClass = Literal["economy", "premium-economy", "business", "first"]
"""Cabin class filter for :meth:`GFlights.search`."""

StopFilter = Literal["all", "nonstop", "one-stop"]
"""Stop count filter for :meth:`GFlights.search`."""

SortOrder = Literal["best", "price", "duration", "departure-time", "arrival-time"]
"""Sort order for :meth:`GFlights.search` results."""
