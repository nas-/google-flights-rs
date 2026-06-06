"""Typed parameter aliases for the gflights API.

The :class:`Currency` enum gives IDE autocompletion for the most common
ISO-4217 codes; the :data:`Literal` aliases do the same for the small
fixed-choice string parameters (cabin class, stop filter, sort order, trip
duration).  Every method also accepts the plain ``str`` form.
"""

from __future__ import annotations

from enum import Enum
from typing import Literal

TravelClass = Literal["economy", "premium-economy", "business", "first"]
"""Cabin class filter for :meth:`Client.search`."""

StopFilter = Literal["all", "nonstop", "one-stop"]
"""Stop count filter for :meth:`Client.search`."""

SortOrder = Literal["best", "price", "duration", "departure-time", "arrival-time"]
"""Sort order for :meth:`Client.search` results."""

Duration = Literal["weekend", "week", "2weeks"]
"""Trip-duration choice for :meth:`Client.explore`."""


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
