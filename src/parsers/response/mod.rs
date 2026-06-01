//! Response parsers: deserialize Google Flights API payloads into typed Rust
//! structs.
//!
//! | Module | Purpose |
//! |---|---|
//! | [`calendar_graph_response`] | Price-graph response |
//! | [`city_response`] | City / location lookup response |
//! | [`date_grid_response`] | Date-grid (departure × return matrix) response |
//! | [`flight_response`] | Flight-search response (itineraries, prices, CO2) |
//! | [`offer_response`] | Booking-offer response (per-OTA prices and URLs) |

pub mod calendar_graph_response;
pub mod city_response;
pub mod date_grid_response;
pub mod flight_response;
pub mod offer_response;
