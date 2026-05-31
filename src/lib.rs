pub mod parsers;
pub mod protos;
pub mod requests;

/// Re-exported for downcasting: `err.downcast_ref::<RateLimitedError>()`.
pub use requests::api::RateLimitedError;
/// Re-exported for configuring retry behaviour on [`requests::api::ApiClient`].
pub use requests::api::RetryConfig;
