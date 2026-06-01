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

    def reset_rate_limit(self) -> None: ...
    def __repr__(self) -> str: ...
