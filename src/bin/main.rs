//! `gflights` — command-line interface for the gflights library.
//!
//! # Usage
//!
//! **One-shot mode** — pass a subcommand directly:
//! ```text
//! gflights search --from LHR --to JFK --date 2025-08-01 [OPTIONS]
//! gflights graph  --from LHR --to JFK --date 2025-08-01 [--months 3]
//! gflights offer  --from LHR --to JFK --date 2025-08-01
//! ```
//!
//! **Interactive mode** — run `gflights` with no arguments to enter a REPL:
//! ```text
//! gflights> search --from LHR --to JFK --date 2025-08-01
//! gflights> graph  --from MXP --to SYD --date 2025-09-01 --months 2
//! gflights> dgrid  --from LHR --to JFK --dep-start 2025-08-01 --dep-end 2025-08-07 --ret-start 2025-08-10 --ret-end 2025-08-17
//! gflights> quit
//! ```

mod cli;

use anyhow::Result;
use clap::Parser;
use cli::{run_command, run_repl, Cli};
use gflights::requests::api::ApiClient;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Minimal tracing — RUST_LOG overrides.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // Create one shared ApiClient (fetches the frontend version once).
    let client = ApiClient::new().await;

    match cli.command {
        Some(cmd) => run_command(cmd, &client).await,
        None => run_repl(&client).await,
    }
}
