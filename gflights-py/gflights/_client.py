"""Pure-Python ergonomic wrapper around the Rust engine (``_gflights._Client``).

The wrapper owns the public surface: explicitly typed signatures, full
docstrings with runnable examples, and input normalization (``datetime.date``
→ ISO string, :class:`Currency` enum → ISO string). The Rust engine does the
actual work; every method normalizes its arguments and then delegates,
returning the engine's awaitable unchanged.

Per-traveller counts are grouped into a :class:`Passengers` object and the
shared result filters into a :class:`SearchFilters` object, so the route-search
methods take a handful of arguments instead of a dozen.

Because the methods are plain (non-``async``) functions that return the
engine's coroutine, argument validation still raises **synchronously** at call
time — before the coroutine is awaited.
"""

from __future__ import annotations

import datetime as _dt
from typing import Awaitable, Optional, Union

from gflights._gflights import (
    CheapDate,
    DateGridEntry,
    DealResult,
    ExploreResult,
    FlightResult,
    Offer,
    PriceEntry,
    _Client,
)
from gflights._types import (
    Currency,
    Duration,
    Passengers,
    SearchFilters,
    SortOrder,
    StopFilter,
    TravelClass,
)

DateLike = Union[str, _dt.date]
"""A date as an ISO ``"YYYY-MM-DD"`` string or a ``datetime.date`` / ``datetime``."""

CurrencyLike = Union[str, Currency]
"""A currency as an ISO-4217 ``str`` or a :class:`Currency` member."""


def _as_date_str(value: DateLike) -> str:
    """Normalize a date argument to an ISO ``"YYYY-MM-DD"`` string.

    Accepts a ``datetime.date`` / ``datetime.datetime`` (formatted to its date
    part) or an already-formatted ``str`` (passed through). Anything else
    raises :exc:`TypeError`.
    """
    if isinstance(value, _dt.datetime):
        return value.date().strftime("%Y-%m-%d")
    if isinstance(value, _dt.date):
        return value.strftime("%Y-%m-%d")
    if isinstance(value, str):
        return value
    raise TypeError(
        f"date must be a str 'YYYY-MM-DD' or datetime.date, got {type(value).__name__}"
    )


def _as_currency_str(value: CurrencyLike) -> str:
    """Normalize a currency argument to an ISO-4217 ``str``."""
    if isinstance(value, Currency):
        return value.value
    return value


class Client:
    """Async Python client for Google Flights, backed by a fast Rust/tokio core.

    Locale (currency / language / country) is fixed per client at construction.
    All search methods are coroutines — ``await`` them, or run several
    concurrently with :func:`asyncio.gather`.

    Example::

        import asyncio
        from gflights import Client

        async def main():
            client = Client(currency="USD", country="US")
            flights = await client.search(
                origin="LHR", destination="JFK", date="2026-08-01",
            )
            for f in flights:
                print(f.airline, f.duration_minutes, f.price)

        asyncio.run(main())
    """

    __slots__ = ("_inner",)

    def __init__(
        self,
        user_agent: Optional[str] = None,
        proxy: Optional[str] = None,
        currency: CurrencyLike = "EUR",
        lang: str = "en",
        country: str = "GB",
    ) -> None:
        """Create a client.

        :param user_agent: Override the User-Agent header. By default a real
            desktop browser string is chosen from a rotating pool per client.
        :param proxy: Route requests through a proxy URL (``http://``,
            ``https://`` or ``socks5://``). ``None`` = direct connection.
        :param currency: ISO-4217 code or :class:`Currency` member applied to
            every request (default ``"EUR"``). Unknown codes raise
            :exc:`ValueError`.
        :param lang: BCP-47 language subtag (default ``"en"``).
        :param country: ISO 3166-1 alpha-2 country code (default ``"GB"``).
        """
        self._inner = _Client(
            user_agent=user_agent,
            proxy=proxy,
            currency=_as_currency_str(currency),
            lang=lang,
            country=country,
        )

    # ------------------------------------------------------------------ search
    def search(
        self,
        origin: str,
        destination: str,
        date: DateLike,
        return_date: Optional[DateLike] = None,
        passengers: Optional[Passengers] = None,
        filters: Optional[SearchFilters] = None,
    ) -> Awaitable[list[FlightResult]]:
        """Search for flights on a route.

        :param origin: Departure IATA code or city name (e.g. ``"LHR"``).
        :param destination: Arrival IATA code or city name.
        :param date: Departure date (``"YYYY-MM-DD"`` or ``datetime.date``).
        :param return_date: Return date for round-trips; ``None`` for one-way.
        :param passengers: Passenger party (see :class:`Passengers`); defaults
            to a single adult.
        :param filters: Result filters (see :class:`SearchFilters`); defaults
            to no filtering. All fields are honoured by ``search``.
        :returns: Coroutine → ``list[FlightResult]``.
        :raises ValueError: on invalid dates, enums, or > 9 passengers.
        :raises gflights.GFlightsError: on network / parse failure.

        Example::

            from gflights import Passengers, SearchFilters
            flights = await client.search(
                origin="LHR", destination="JFK",
                date=datetime.date(2026, 8, 1),
                passengers=Passengers(adults=2),
                filters=SearchFilters(stops="nonstop"),
            )
        """
        p = passengers or Passengers()
        f = filters or SearchFilters()
        return self._inner.search(
            from_airport=origin,
            to_airport=destination,
            date=_as_date_str(date),
            return_date=None if return_date is None else _as_date_str(return_date),
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=f.travel_class,
            stops=f.stops,
            sort=f.sort,
            airlines_include=f.airlines_include,
            airlines_exclude=f.airlines_exclude,
            via=f.via,
            lower_emissions=f.lower_emissions,
            max_price=f.max_price,
            carry_on=f.carry_on,
            checked_bags=f.checked_bags,
        )

    # ------------------------------------------------------------- price_graph
    def price_graph(
        self,
        origin: str,
        destination: str,
        date: DateLike,
        months: int = 1,
        passengers: Optional[Passengers] = None,
        filters: Optional[SearchFilters] = None,
    ) -> Awaitable[list[PriceEntry]]:
        """Cheapest fare per day over a date range (price graph).

        :param origin: Departure IATA code or city name.
        :param destination: Arrival IATA code or city name.
        :param date: Start date (``"YYYY-MM-DD"`` or ``datetime.date``).
        :param months: Months of price data to fetch (default 1).
        :param passengers: Passenger party (see :class:`Passengers`).
        :param filters: Result filters (see :class:`SearchFilters`); ``sort``
            is ignored (entries are always returned by date).
        :returns: Coroutine → ``list[PriceEntry]``, sorted by date.

        Example::

            graph = await client.price_graph(
                origin="LHR", destination="JFK", date="2026-08-01", months=2,
            )
        """
        p = passengers or Passengers()
        f = filters or SearchFilters()
        return self._inner.price_graph(
            from_airport=origin,
            to_airport=destination,
            date=_as_date_str(date),
            months=months,
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=f.travel_class,
            stops=f.stops,
            airlines_include=f.airlines_include,
            airlines_exclude=f.airlines_exclude,
            via=f.via,
            lower_emissions=f.lower_emissions,
            max_price=f.max_price,
            carry_on=f.carry_on,
            checked_bags=f.checked_bags,
        )

    # --------------------------------------------------------------- date_grid
    def date_grid(
        self,
        origin: str,
        destination: str,
        dep_start: DateLike,
        dep_end: DateLike,
        ret_start: DateLike,
        ret_end: DateLike,
        passengers: Optional[Passengers] = None,
        filters: Optional[SearchFilters] = None,
    ) -> Awaitable[list[DateGridEntry]]:
        """Cheapest fare for every (departure × return) date combination.

        :param origin: Departure IATA code or city name.
        :param destination: Arrival IATA code or city name.
        :param dep_start: First outbound departure date.
        :param dep_end: Last outbound departure date.
        :param ret_start: First return date.
        :param ret_end: Last return date.
        :param passengers: Passenger party (see :class:`Passengers`).
        :param filters: Result filters (see :class:`SearchFilters`); ``sort``
            is ignored (cells are keyed by date pair).
        :returns: Coroutine → ``list[DateGridEntry]``.

        Example::

            grid = await client.date_grid(
                origin="LHR", destination="CDG",
                dep_start="2026-08-01", dep_end="2026-08-05",
                ret_start="2026-08-10", ret_end="2026-08-14",
            )
        """
        p = passengers or Passengers()
        f = filters or SearchFilters()
        return self._inner.date_grid(
            from_airport=origin,
            to_airport=destination,
            dep_start=_as_date_str(dep_start),
            dep_end=_as_date_str(dep_end),
            ret_start=_as_date_str(ret_start),
            ret_end=_as_date_str(ret_end),
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=f.travel_class,
            stops=f.stops,
            airlines_include=f.airlines_include,
            airlines_exclude=f.airlines_exclude,
            via=f.via,
            lower_emissions=f.lower_emissions,
            max_price=f.max_price,
            carry_on=f.carry_on,
            checked_bags=f.checked_bags,
        )

    # -------------------------------------------------------- multi_city_search
    def multi_city_search(
        self,
        legs: list[tuple[str, str, DateLike]],
        passengers: Optional[Passengers] = None,
        travel_class: TravelClass = "economy",
        sort: SortOrder = "best",
        max_price: Optional[int] = None,
        carry_on: int = 0,
        checked_bags: int = 0,
    ) -> Awaitable[list[FlightResult]]:
        """Search across multiple legs (open-jaw / multi-city).

        The multi-city endpoint supports a reduced filter set, so the filters
        are passed inline rather than via :class:`SearchFilters` (no
        ``stops`` / ``airlines`` / ``via`` / ``lower_emissions``).

        :param legs: List of ``(origin, destination, date)`` tuples (minimum 2).
            Each date is a ``str`` or ``datetime.date``.
        :param passengers: Passenger party (see :class:`Passengers`).
        :param travel_class: Cabin class (default ``"economy"``).
        :param sort: Result ordering (default ``"best"``).
        :param max_price: Maximum price cap in the client currency.
        :param carry_on: Carry-on bags required (0 = no restriction).
        :param checked_bags: Checked bags required (0 = no restriction).
        :returns: Coroutine → ``list[FlightResult]``.
        :raises ValueError: with fewer than 2 legs.

        Example::

            flights = await client.multi_city_search([
                ("LHR", "JFK", "2026-08-01"),
                ("JFK", "LAX", "2026-08-08"),
            ])
        """
        p = passengers or Passengers()
        norm_legs = [(o, d, _as_date_str(dt)) for (o, d, dt) in legs]
        return self._inner.multi_city_search(
            legs=norm_legs,
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=travel_class,
            sort=sort,
            max_price=max_price,
            carry_on=carry_on,
            checked_bags=checked_bags,
        )

    # ----------------------------------------------------------------- explore
    def explore(
        self,
        origin: str,
        month: Optional[int] = None,
        duration: Duration = "week",
        max_price: Optional[int] = None,
        interest: Optional[str] = None,
        max_flight_hours: Optional[int] = None,
        carry_on: int = 0,
        checked: int = 0,
        passengers: Optional[Passengers] = None,
        travel_class: TravelClass = "economy",
    ) -> Awaitable[list[ExploreResult]]:
        """Explore cheap destinations from an origin airport.

        :param origin: Origin IATA code or city name (e.g. ``"LUX"``).
        :param month: Calendar month (1–12) to search in; ``None`` for any.
        :param duration: ``"weekend"`` / ``"week"`` / ``"2weeks"``.
        :param max_price: Maximum total round-trip price.
        :param interest: Interest name (e.g. ``"beaches"``), an alias, or a raw
            ``/m/…`` MID. Unknown values raise :exc:`ValueError`.
        :param max_flight_hours: Maximum one-way flight time in hours.
        :param carry_on: Carry-on bags required (0 = no restriction).
        :param checked: Checked bags required (0 = no restriction).
        :param passengers: Passenger party (see :class:`Passengers`).
        :param travel_class: Cabin class (default ``"economy"``).
        :returns: Coroutine → ``list[ExploreResult]``.

        Example::

            dests = await client.explore(origin="LUX", interest="beaches")
        """
        p = passengers or Passengers()
        return self._inner.explore(
            from_airport=origin,
            month=month,
            duration=duration,
            max_price=max_price,
            interest=interest,
            max_flight_hours=max_flight_hours,
            carry_on=carry_on,
            checked=checked,
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=travel_class,
        )

    # ------------------------------------------------------------------- deals
    def deals(
        self,
        origin: str,
        date: DateLike,
        return_date: DateLike,
        stops: StopFilter = "all",
        max_hours: Optional[int] = None,
        passengers: Optional[Passengers] = None,
        travel_class: TravelClass = "economy",
    ) -> Awaitable[list[DealResult]]:
        """Find discounted destinations (flight deals) from an origin.

        The ``date`` / ``return_date`` pair is a trip-length anchor; the
        endpoint returns deals of similar length across many dates.

        :param origin: Origin IATA code or city name.
        :param date: Outbound date (trip-length anchor).
        :param return_date: Return date.
        :param stops: Stop-count filter. The deals endpoint only distinguishes
            ``"nonstop"`` from any-stops, so ``"one-stop"`` is treated as
            ``"all"`` (default ``"all"``).
        :param max_hours: Maximum one-way flight time in hours.
        :param passengers: Passenger party (see :class:`Passengers`).
        :param travel_class: Cabin class (default ``"economy"``).
        :returns: Coroutine → ``list[DealResult]``.

        Example::

            deals = await client.deals(
                origin="LUX", date="2026-06-20", return_date="2026-06-24",
            )
        """
        p = passengers or Passengers()
        return self._inner.deals(
            from_airport=origin,
            out=_as_date_str(date),
            ret=_as_date_str(return_date),
            nonstop=stops in ("nonstop", "non-stop", "direct"),
            max_hours=max_hours,
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=travel_class,
        )

    # ---------------------------------------------------------- cheapest_dates
    def cheapest_dates(
        self,
        origin: str,
        destination: str,
        date: DateLike,
        months: int = 3,
        trip_duration_days: Optional[int] = None,
        passengers: Optional[Passengers] = None,
        filters: Optional[SearchFilters] = None,
    ) -> Awaitable[list[CheapDate]]:
        """Find the cheapest departure dates over a range of months.

        :param origin: Departure IATA code or city name.
        :param destination: Arrival IATA code or city name.
        :param date: Start of the search window.
        :param months: Number of months to scan (default 3).
        :param trip_duration_days: Round-trip length in days; ``None`` for
            one-way date discovery.
        :param passengers: Passenger party (see :class:`Passengers`).
        :param filters: Result filters (see :class:`SearchFilters`); ``sort``
            is ignored (results are always cheapest-first).
        :returns: Coroutine → ``list[CheapDate]``, sorted cheapest first.

        Example::

            dates = await client.cheapest_dates(
                origin="LHR", destination="JFK",
                date="2026-08-01", months=3, trip_duration_days=7,
            )
        """
        p = passengers or Passengers()
        f = filters or SearchFilters()
        return self._inner.cheapest_dates(
            from_airport=origin,
            to_airport=destination,
            date=_as_date_str(date),
            months=months,
            trip_duration_days=trip_duration_days,
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=f.travel_class,
            stops=f.stops,
            airlines_include=f.airlines_include,
            airlines_exclude=f.airlines_exclude,
            via=f.via,
            lower_emissions=f.lower_emissions,
            max_price=f.max_price,
            carry_on=f.carry_on,
            checked_bags=f.checked_bags,
        )

    # ------------------------------------------------------------------- offer
    def offer(
        self,
        origin: str,
        destination: str,
        date: DateLike,
        return_date: Optional[DateLike] = None,
        passengers: Optional[Passengers] = None,
        filters: Optional[SearchFilters] = None,
    ) -> Awaitable[list[Offer]]:
        """Price the cheapest itinerary and return its booking offers.

        Searches, locks in the cheapest outbound (and return for round trips),
        fetches booking offers and resolves each one's booking URL.

        :param origin: Departure IATA code or city name.
        :param destination: Arrival IATA code or city name.
        :param date: Departure date (``"YYYY-MM-DD"`` or ``datetime.date``).
        :param return_date: Return date for round-trips; ``None`` for one-way.
        :param passengers: Passenger party (see :class:`Passengers`).
        :param filters: Result filters (see :class:`SearchFilters`).
        :returns: Coroutine → ``list[Offer]``, cheapest first.

        Example::

            offers = await client.offer(
                origin="LHR", destination="JFK", date="2026-08-01",
            )
            print(offers[0].price, offers[0].booking_url)
        """
        p = passengers or Passengers()
        f = filters or SearchFilters()
        return self._inner.offer(
            from_airport=origin,
            to_airport=destination,
            date=_as_date_str(date),
            return_date=None if return_date is None else _as_date_str(return_date),
            adults=p.adults,
            children=p.children,
            infants_in_seat=p.infants_in_seat,
            infants_on_lap=p.infants_on_lap,
            travel_class=f.travel_class,
            stops=f.stops,
            sort=f.sort,
            airlines_include=f.airlines_include,
            airlines_exclude=f.airlines_exclude,
            via=f.via,
            lower_emissions=f.lower_emissions,
            max_price=f.max_price,
            carry_on=f.carry_on,
            checked_bags=f.checked_bags,
        )

    # -------------------------------------------------------------- rate limit
    @property
    def rate_limited(self) -> bool:
        """``True`` if the last request was rate-limited by Google (HTTP 429)."""
        return self._inner.rate_limited

    def reset_rate_limit(self) -> None:
        """Reset the rate-limit flag after a cooling-off period."""
        self._inner.reset_rate_limit()

    def __repr__(self) -> str:
        return "Client()"
