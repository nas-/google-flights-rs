"""Type stubs for the compiled Rust extension gflights._gflights."""

from typing import Optional

class GFlightsError(Exception):
    """Exception raised by all gflights API methods on network or parse errors.

    Catch this to handle errors without matching on :exc:`RuntimeError` or
    :exc:`ValueError`::

        try:
            flights = await client.search(...)
        except gflights.GFlightsError as e:
            print(f"search failed: {e}")
    """
    ...

class LegInfo:
    from_airport: str
    to_airport: str
    departure_time: str
    arrival_time: str
    departure_date: str
    arrival_date: str
    duration_minutes: Optional[int]
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class LayoverInfo:
    connection_minutes: int
    arrival_airport: str
    departure_airport: str
    overnight: bool
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class EmissionsInfo:
    vs_average_percent: Optional[int]
    co2_this_flight_g: Optional[int]
    co2_typical_route_g: Optional[int]
    co2_lowest_route_g: Optional[int]
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

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
    def to_dict(self) -> dict: ...

class PriceEntry:
    date: str
    price: int
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class DateGridEntry:
    dep_date: str
    ret_date: str
    price: int
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class CheapDate:
    """One result from :meth:`GFlights.cheapest_dates`, sorted cheapest-first.

    ``return_date`` is ``None`` for one-way results and set for round-trip results.
    """
    departure_date: str
    return_date: Optional[str]
    price: int
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class ExploreResult:
    """One destination returned by :meth:`GFlights.explore`."""
    place_id: str
    name: str
    country: str
    lat: float
    lng: float
    image_url: Optional[str]
    nearest_airport: str
    flight_airport: Optional[str]
    date_from: Optional[str]
    date_to: Optional[str]
    price: Optional[int]
    airline: Optional[str]
    stops: Optional[int]
    flight_duration_minutes: Optional[int]
    accommodation_price: Optional[int]
    booking_token: str
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class DealResult:
    """One discounted destination returned by :meth:`GFlights.deals`."""
    origin_iata: str
    destination_iata: str
    destination_city: str
    destination_country: str
    destination_mid: Optional[str]
    outbound_date: Optional[str]
    return_date: Optional[str]
    price: Optional[int]
    typical_price: Optional[int]
    discount_pct: Optional[int]
    duration_minutes: Optional[int]
    stops: Optional[int]
    airline_code: Optional[str]
    airline_name: Optional[str]
    image_url: Optional[str]
    highlights: list[str]
    description: Optional[str]
    booking_url: Optional[str]
    booking_token: Optional[str]
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class BookingOption:
    """One booking channel (OTA / partner) inside an :class:`Offer`."""
    partner_names: list[str]
    price: Optional[int]
    booking_url: Optional[str]
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class Offer:
    """One priced booking option returned by :meth:`GFlights.offer`."""
    airline_names: list[str]
    price: Optional[int]
    booking_url: Optional[str]
    @property
    def sub_options(self) -> list[BookingOption]: ...
    def __repr__(self) -> str: ...
    def to_dict(self) -> dict: ...

class _Client:
    """Internal Rust engine. Use the public :class:`gflights.Client` wrapper.

    The constructor is synchronous (fast — just initialises the HTTP client).
    All search methods are async coroutines that integrate directly with
    asyncio without spawning threads.
    """

    rate_limited: bool

    def __init__(
        self,
        user_agent: Optional[str] = None,
        proxy: Optional[str] = None,
        currency: str = ...,
        lang: str = ...,
        country: str = ...,
    ) -> None: ...

    # Methods return asyncio.Future objects (awaitable); typed as coroutines for IDE support.
    async def search(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        return_date: Optional[str] = ...,
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
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
    ) -> list[FlightResult]:
        """Search for flights. Returns a coroutine."""
        ...

    async def price_graph(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        months: int = ...,
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
        stops: str = ...,
        airlines_include: list[str] = ...,
        airlines_exclude: list[str] = ...,
        via: list[str] = ...,
        lower_emissions: bool = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
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
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
        stops: str = ...,
        airlines_include: list[str] = ...,
        airlines_exclude: list[str] = ...,
        via: list[str] = ...,
        lower_emissions: bool = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
    ) -> list[DateGridEntry]:
        """Departure × return price matrix. Returns a coroutine."""
        ...

    async def multi_city_search(
        self,
        legs: list[tuple[str, str, str]],
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
        sort: str = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
    ) -> list[FlightResult]:
        """Multi-city (open-jaw) search. Each leg is ``(from, to, "YYYY-MM-DD")``."""
        ...

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
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
    ) -> list[ExploreResult]:
        """Explore cheap destinations from an origin airport.

        Returns a coroutine → ``list[ExploreResult]``.
        """
        ...

    async def deals(
        self,
        from_airport: str,
        out: str,
        ret: str,
        nonstop: bool = ...,
        max_hours: Optional[int] = ...,
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
    ) -> list[DealResult]:
        """Find discounted destinations (flight deals) from an origin.

        Returns a coroutine → ``list[DealResult]``.
        """
        ...

    async def cheapest_dates(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        months: int = ...,
        trip_duration_days: Optional[int] = ...,
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
        travel_class: str = ...,
        stops: str = ...,
        airlines_include: list[str] = ...,
        airlines_exclude: list[str] = ...,
        via: list[str] = ...,
        lower_emissions: bool = ...,
        max_price: Optional[int] = ...,
        carry_on: int = ...,
        checked_bags: int = ...,
    ) -> list[CheapDate]:
        """Find cheapest departure dates sorted by price.

        Pass ``trip_duration_days=N`` for round-trip fixed-length results;
        omit (or pass ``None``) for one-way date discovery.
        Returns a coroutine.
        """
        ...

    async def offer(
        self,
        from_airport: str,
        to_airport: str,
        date: str,
        return_date: Optional[str] = ...,
        adults: int = ...,
        children: int = ...,
        infants_in_seat: int = ...,
        infants_on_lap: int = ...,
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
    ) -> list[Offer]:
        """Price the cheapest itinerary and return booking offers.

        Searches, locks in the cheapest outbound (and return for round trips),
        fetches booking offers and resolves each one's booking URL.
        Returns a coroutine → ``list[Offer]``, cheapest first.
        """
        ...

    def reset_rate_limit(self) -> None: ...
    def __repr__(self) -> str: ...
