//! Parsers and request-builders for the Google Flights API.
//!
//! The module is split into two groups:
//!
//! - **[`request`]** — encode `Config` fields into the POST body format
//!   expected by Google Flights endpoints.
//! - **[`response`]** — deserialize raw API payloads into typed Rust structs.
//!
//! Shared primitives used by both groups live in [`common`].
//!
//! ## Backward-compatibility aliases
//!
//! The individual parser modules are also re-exported at the `parsers::*`
//! level (e.g. `parsers::flight_response`) so that existing call-sites do not
//! need to change when the sub-module layout evolves.

#![deny(clippy::unwrap_used)]

pub mod common;
pub mod constants;

pub mod request;
pub mod response;

// Re-export sub-modules at the old flat path so external crates that already
// use `gflights::parsers::flight_response::…` continue to compile unchanged.
pub use request::calendar_graph_request;
pub use request::city_request;
pub use request::date_grid_request;
pub use request::flight_request;
pub use response::calendar_graph_response;
pub use response::city_response;
pub use response::date_grid_response;
pub use response::flight_response;
pub use response::offer_response;
