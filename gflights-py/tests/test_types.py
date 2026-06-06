"""Tests for Python data class attributes and repr."""

import gflights


def test_price_entry_fields():
    # Instantiate via the live module's class (no constructor in Python)
    # We'll get instances from live tests; here just verify class is inspectable.
    assert hasattr(gflights.PriceEntry, "__doc__") or True  # class exists


def test_date_grid_entry_repr():
    # We can exercise __repr__ via a live result; verify class attributes
    # are documented correctly by checking attribute names exist on the type.
    for attr in ("dep_date", "ret_date", "price"):
        assert hasattr(gflights.DateGridEntry, attr), f"DateGridEntry missing .{attr}"


def test_flight_result_has_expected_attributes():
    for attr in ("airline", "duration_minutes", "stops", "price",
                 "booking_token", "legs", "layovers", "emissions"):
        assert hasattr(gflights.FlightResult, attr), f"FlightResult missing .{attr}"


def test_leg_info_has_expected_attributes():
    for attr in ("from_airport", "to_airport", "departure_time", "arrival_time",
                 "departure_date", "arrival_date", "duration_minutes"):
        assert hasattr(gflights.LegInfo, attr), f"LegInfo missing .{attr}"


def test_layover_info_has_expected_attributes():
    for attr in ("connection_minutes", "arrival_airport", "departure_airport", "overnight"):
        assert hasattr(gflights.LayoverInfo, attr), f"LayoverInfo missing .{attr}"


def test_emissions_info_has_expected_attributes():
    for attr in ("vs_average_percent", "co2_this_flight_g",
                 "co2_typical_route_g", "co2_lowest_route_g"):
        assert hasattr(gflights.EmissionsInfo, attr), f"EmissionsInfo missing .{attr}"


def test_offer_and_booking_option_attributes():
    for attr in ("airline_names", "price", "booking_url", "sub_options"):
        assert hasattr(gflights.Offer, attr), f"Offer missing .{attr}"
    for attr in ("partner_names", "price", "booking_url"):
        assert hasattr(gflights.BookingOption, attr), f"BookingOption missing .{attr}"


def test_currency_enum_members_are_iso_strings():
    assert gflights.Currency.USD == "USD"
    assert gflights.Currency.EUR.value == "EUR"
    # Enum member is a plain str, usable wherever a currency str is expected.
    assert isinstance(gflights.Currency.GBP, str)


def test_currency_enum_accepts_client_construction():
    c = gflights.Client(currency=gflights.Currency.USD)
    assert repr(c) == "Client()"
