"""Typed parameter aliases for the gflights API.

The :class:`Currency` enum gives IDE autocompletion for the most common
ISO-4217 codes; the :data:`Literal` aliases do the same for the small
fixed-choice string parameters (cabin class, stop filter, sort order, trip
duration).  Every method also accepts the plain ``str`` form.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import List, Literal, Optional

TravelClass = Literal["economy", "premium-economy", "business", "first"]
"""Cabin class filter for :meth:`Client.search`."""

StopFilter = Literal["all", "nonstop", "one-stop"]
"""Stop count filter for :meth:`Client.search`."""

SortOrder = Literal["best", "price", "duration", "departure-time", "arrival-time"]
"""Sort order for :meth:`Client.search` results."""

Duration = Literal["weekend", "week", "2weeks"]
"""Trip-duration choice for :meth:`Client.explore`."""


@dataclass
class Passengers:
    """The passenger party for a search.

    Group the per-traveller counts into one argument instead of passing four
    separate integers to every method. The total must be between 1 and 9 with
    at least one adult; the engine raises :exc:`ValueError` otherwise.

    :param adults: Adults aged 12+ (default 1).
    :param children: Children aged 2–11 (default 0).
    :param infants_in_seat: Infants in their own seat (default 0).
    :param infants_on_lap: Lap infants (default 0).

    Example::

        from gflights import Passengers
        await client.search(origin="LHR", destination="JFK", date="2026-08-01",
                            passengers=Passengers(adults=2, children=1))
    """

    adults: int = 1
    children: int = 0
    infants_in_seat: int = 0
    infants_on_lap: int = 0


@dataclass
class SearchFilters:
    """Result filters shared by the route-search endpoints.

    Group the optional filters into one argument instead of passing them
    individually. Pass only the fields you care about; the rest keep their
    defaults.

    .. note::
       Not every endpoint honours every field. :meth:`Client.price_graph`,
       :meth:`Client.date_grid` and :meth:`Client.cheapest_dates` ignore
       ``sort`` (their results have a fixed order).

    :param travel_class: Cabin class (default ``"economy"``).
    :param stops: Stop-count filter (default ``"all"``).
    :param sort: Result ordering (default ``"best"``).
    :param airlines_include: IATA codes or alliances to include
        (e.g. ``["BA", "ONEWORLD"]``).
    :param airlines_exclude: IATA codes or alliances to exclude.
    :param via: Require a connection through these airports.
    :param lower_emissions: Restrict to below-average CO₂ flights.
    :param max_price: Maximum price cap in the client currency.
    :param carry_on: Carry-on bags required (0 = no restriction).
    :param checked_bags: Checked bags required (0 = no restriction).

    Example::

        from gflights import SearchFilters
        await client.search(origin="LHR", destination="JFK", date="2026-08-01",
                            filters=SearchFilters(stops="nonstop", sort="price"))
    """

    travel_class: TravelClass = "economy"
    stops: StopFilter = "all"
    sort: SortOrder = "best"
    airlines_include: List[str] = field(default_factory=list)
    airlines_exclude: List[str] = field(default_factory=list)
    via: List[str] = field(default_factory=list)
    lower_emissions: bool = False
    max_price: Optional[int] = None
    carry_on: int = 0
    checked_bags: int = 0


class Currency(str, Enum):
    """ISO-4217 currency codes accepted by :class:`Client`.

    Members are plain strings (``Currency.USD == "USD"``), so they can be
    passed anywhere a currency ``str`` is expected.
    """

    AED = "AED"
    ALL = "ALL"
    AMD = "AMD"
    ARS = "ARS"
    AUD = "AUD"
    AWG = "AWG"
    AZN = "AZN"
    BAM = "BAM"
    BGN = "BGN"
    BHD = "BHD"
    BMD = "BMD"
    BRL = "BRL"
    BSD = "BSD"
    BYN = "BYN"
    CAD = "CAD"
    CHF = "CHF"
    CLP = "CLP"
    CNY = "CNY"
    COP = "COP"
    CRC = "CRC"
    CUP = "CUP"
    CZK = "CZK"
    DKK = "DKK"
    DOP = "DOP"
    DZD = "DZD"
    EGP = "EGP"
    EUR = "EUR"
    GBP = "GBP"
    GEL = "GEL"
    HKD = "HKD"
    HUF = "HUF"
    IDR = "IDR"
    ILS = "ILS"
    INR = "INR"
    IRR = "IRR"
    ISK = "ISK"
    JMD = "JMD"
    JOD = "JOD"
    JPY = "JPY"
    KRW = "KRW"
    KWD = "KWD"
    KZT = "KZT"
    LBP = "LBP"
    MAD = "MAD"
    MDL = "MDL"
    MKD = "MKD"
    MXN = "MXN"
    MYR = "MYR"
    NOK = "NOK"
    NZD = "NZD"
    OMR = "OMR"
    PAB = "PAB"
    PEN = "PEN"
    PHP = "PHP"
    PKR = "PKR"
    PLN = "PLN"
    QAR = "QAR"
    RON = "RON"
    RSD = "RSD"
    RUB = "RUB"
    SAR = "SAR"
    SEK = "SEK"
    SGD = "SGD"
    THB = "THB"
    TRY = "TRY"
    TWD = "TWD"
    UAH = "UAH"
    USD = "USD"
    VND = "VND"
    XPF = "XPF"
    ZAR = "ZAR"
