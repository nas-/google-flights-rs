"""Type stubs for the compiled Rust extension gflights._gflights."""

from typing import Optional

class LegInfo:
    from_airport: str
    to_airport: str
    departure_time: str
    arrival_time: str
    departure_date: str
    arrival_date: str
    duration_minutes: Optional[int]
    def __repr__(self) -> str: ...

class LayoverInfo:
    connection_minutes: int
    arrival_airport: str
    departure_airport: str
    overnight: bool
    def __repr__(self) -> str: ...

class EmissionsInfo:
    vs_average_percent: Optional[int]
    co2_this_flight_g: Optional[int]
    co2_typical_route_g: Optional[int]
    co2_lowest_route_g: Optional[int]
    def __repr__(self) -> str: ...

class FlightResult:
    airline: str
    duration_minutes: int
    stops: int
    price: Optional[int]
    booking_token: str
    @property
    def legs(self) -> list[LegInfo]: ...
    @property
    def layovers(self) -> list[LayoverInfo]: ...
    @property
    def emissions(self) -> Optional[EmissionsInfo]: ...
    def __repr__(self) -> str: ...

class PriceEntry:
    date: str
    price: int
    def __repr__(self) -> str: ...

class DateGridEntry:
    dep_date: str
    ret_date: str
    price: int
    def __repr__(self) -> str: ...

class CheapDate:
    """One result from :meth:`GFlights.cheapest_dates`, sorted cheapest-first.

    ``return_date`` is ``None`` for one-way results and set for round-trip results.
    """
    departure_date: str
    return_date: Optional[str]
    price: int
    def __repr__(self) -> str: ...

<<<<<<< HEAD
class ExploreResult:
    """One destination returned by :meth:`GFlights.explore`."""
    place_id: str
    name: str
    country: str
    lat: float
    lng: float
    image_url: Optional[str]
    nearest_airport: str
    date_from: Optional[str]
    date_to: Optional[str]
    price: Optional[int]
    airline: Optional[str]
    stops: Optional[int]
    flight_duration_minutes: Optional[int]
    accommodation_price: Optional[int]
    booking_token: str
    def __repr__(self) -> str: ...

=======
>>>>>>> feat/cheapest-dates
class GFlights:
    """Async Python client for Google Flights, backed by Rust/tokio.

    The constructor is synchronous (fast — just initialises the HTTP client).
    All search methods are async coroutines that integrate directly with
    asyncio without spawning threads.
    """

    rate_limited: bool

    def __init__(self) -> None: ...

    # Methods return asyncio.Future objects (awaitable); typed as coroutines for IDE support.
    async def search(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        return_date: Optional[str] = ...,
        adults: int = ...,
        travel_class: str = ...,
        stops: str = ...,
        sort: str = ...,
        airlines_include: list[str] = ...,
        airlines_exclude: list[str] = ...,
        via: list[str] = ...,
        lower_emissions: bool = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[FlightResult]:
        """Search for flights. Returns a coroutine."""
        ...

    async def price_graph(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        months: int = ...,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[PriceEntry]:
        """Cheapest fare per day over a date range. Returns a coroutine."""
        ...

    async def date_grid(
        self,
        from_airport: str,
        to_airport: str,
        dep_start: str,
        dep_end: str,
        ret_start: str,
        ret_end: str,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[DateGridEntry]:
        """Departure × return price matrix. Returns a coroutine."""
        ...

    async def multi_city_search(
        self,
        legs: list[tuple[str, str, str]],
        adults: int = ...,
        travel_class: str = ...,
        sort: str = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[FlightResult]:
        """Multi-city (open-jaw) search. Each leg is ``(from, to, "YYYY-MM-DD")``."""
        ...

<<<<<<< HEAD
    async def explore(
        self,
        from_airport: str,
        month: Optional[int] = ...,
        duration: str = ...,
        max_price: Optional[int] = ...,
        interest: Optional[str] = ...,
        max_flight_hours: Optional[int] = ...,
        carry_on: int = ...,
        checked: int = ...,
        adults: int = ...,
        travel_class: str = ...,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[ExploreResult]:
        """Explore cheap destinations from an origin airport.

        Returns a coroutine → ``list[ExploreResult]``.
        """
        ...

=======
>>>>>>> feat/cheapest-dates
    async def cheapest_dates(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        months: int = ...,
        trip_duration_days: Optional[int] = ...,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> list[CheapDate]:
        """Find cheapest departure dates sorted by price.

        Pass ``trip_duration_days=N`` for round-trip fixed-length results;
        omit (or pass ``None``) for one-way date discovery.
        Returns a coroutine.
        """
        ...

    def reset_rate_limit(self) -> None: ...
    def __repr__(self) -> str: ...
