//! Request-building helpers: encode `Config` fields into Google Flights
//! POST body strings.
//!
//! | Module | Purpose |
//! |---|---|
//! | [`calendar_graph_request`] | Price-graph (calendar) request body |
//! | [`city_request`] | City / location lookup request body |
//! | [`flight_request`] | Flight-search and booking-offer request body |

pub mod calendar_graph_request;
pub mod city_request;
pub mod flight_request;
