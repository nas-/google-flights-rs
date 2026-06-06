pub const CALENDAR_GRAPH: &str= "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetCalendarGraph";
pub const FLIGHT_REQUEST: &str= "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetShoppingResults";
pub const BOOKING_REQUEST: &str = "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetBookingResults";
pub const BATCHEXECUTE: &str = "https://www.google.com/_/FlightsFrontendUi/data/batchexecute";
/// Date-grid endpoint: returns a matrix of (departure_date × return_date) prices.
pub const CALENDAR_GRID: &str = "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetCalendarGrid";
pub const FLIGHTS_MAIN_PAGE: &str = "https://www.google.com/travel/flights";
/// Click-tracker endpoint: POST `u=<click_token>` to get an HTML meta-refresh
/// that redirects to the actual airline / OTA booking page.
pub const CLK_URL: &str = "https://www.google.com/travel/clk/f";
/// Explore-destinations endpoint: returns cheap destinations from a given origin.
pub const EXPLORE_URL: &str = "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetExploreDestinations";
/// Flight-deals endpoint: returns discounted destinations (price vs typical) from a given origin.
pub const FLIGHT_DEALS_URL: &str = "https://www.google.com/_/FlightsFrontendUi/data/travel.frontend.flights.FlightsFrontendService/GetFlightDealsStreaming";
