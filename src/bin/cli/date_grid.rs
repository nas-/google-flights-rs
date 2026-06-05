use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use gflights::parsers::common::{StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::Config;

use super::OutputFormat;

/// Arguments for the `dgrid` subcommand.
#[derive(Parser, Debug)]
pub struct DateGridArgs {
    /// Departure airport IATA code or city name.
    #[arg(long)]
    pub from: String,

    /// Destination airport IATA code or city name.
    #[arg(long)]
    pub to: String,

    /// First day of the outbound departure window (YYYY-MM-DD).
    #[arg(long)]
    pub dep_start: NaiveDate,

    /// Last day of the outbound departure window (YYYY-MM-DD).
    #[arg(long)]
    pub dep_end: NaiveDate,

    /// First day of the return window (YYYY-MM-DD).
    #[arg(long)]
    pub ret_start: NaiveDate,

    /// Last day of the return window (YYYY-MM-DD).
    #[arg(long)]
    pub ret_end: NaiveDate,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    pub class: TravelClass,

    /// Stop filter.
    #[arg(long, default_value = "all")]
    pub stops: StopOptions,

    /// Output format.
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
}

pub async fn cmd_date_grid(args: DateGridArgs, client: &ApiClient) -> Result<()> {
    let travelers = Travelers::new(vec![args.adults as i32, 0, 0, 0])?;

    let config = Config::builder()
        .departure(&args.from, client)
        .await?
        .destination(&args.to, client)
        .await?
        // Use dep_start as the nominal departing date (required by Config).
        .departing_date(args.dep_start)
        .return_date(args.ret_end)
        .travelers(travelers)
        .travel_class(args.class)
        .stop_options(args.stops)
        .build()?;

    let grid = client
        .request_date_grid(
            &config,
            args.dep_start,
            args.dep_end,
            args.ret_start,
            args.ret_end,
        )
        .await?;

    if grid.entries.is_empty() {
        eprintln!("No price data found for the requested date windows.");
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&grid)?);
        }
        OutputFormat::Table => {
            print!("{grid}");
            if let Some(c) = grid.cheapest() {
                println!(
                    "\nCheapest: {} → {} at {}",
                    c.departure_date, c.return_date, c.price
                );
            }
        }
    }
    Ok(())
}
