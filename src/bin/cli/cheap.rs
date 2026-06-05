use anyhow::Result;
use chrono::{Months, NaiveDate};
use clap::Parser;
use gflights::parsers::common::{TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::Config;

use super::OutputFormat;

/// Arguments for the `cheap` subcommand.
#[derive(Parser, Debug)]
pub struct CheapArgs {
    /// Departure airport IATA code or city name (e.g. LUX, "London").
    #[arg(long)]
    pub from: String,

    /// Destination airport IATA code or city name (e.g. JFK, "New York").
    #[arg(long)]
    pub to: String,

    /// Start of the search window (YYYY-MM-DD).
    #[arg(long)]
    pub date: NaiveDate,

    /// Number of months to scan from --date. [default: 3]
    #[arg(long, default_value = "3")]
    pub months: u32,

    /// Fixed round-trip duration in days (e.g. 7). Omit for one-way results.
    #[arg(long)]
    pub trip_days: Option<u32>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    pub class: TravelClass,

    /// Output format.
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
}

pub async fn cmd_cheap(args: CheapArgs, client: &ApiClient) -> Result<()> {
    let travelers = Travelers::new(vec![args.adults as i32, 0, 0, 0])?;
    let config = Config::builder()
        .departure(&args.from, client)
        .await?
        .destination(&args.to, client)
        .await?
        .departing_date(args.date)
        .travelers(travelers)
        .travel_class(args.class)
        .build()?;

    let results = client
        .cheapest_dates(&config, Months::new(args.months), args.trip_days)
        .await?;

    if results.is_empty() {
        eprintln!("No dates found.");
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        OutputFormat::Table => {
            let is_roundtrip = args.trip_days.is_some();
            if is_roundtrip {
                println!("{:<12}  {:<12}  {:>6}", "DEP DATE", "RET DATE", "PRICE");
            } else {
                println!("{:<12}  {:>6}", "DEP DATE", "PRICE");
            }
            println!("{}", "-".repeat(if is_roundtrip { 34 } else { 20 }));
            for r in &results {
                if let Some(ret) = r.return_date {
                    println!("{:<12}  {:<12}  {:>6}", r.departure_date, ret, r.price);
                } else {
                    println!("{:<12}  {:>6}", r.departure_date, r.price);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cheap_args_parse_one_way() {
        let args = CheapArgs::try_parse_from([
            "cheap",
            "--from",
            "LUX",
            "--to",
            "JFK",
            "--date",
            "2026-09-01",
        ])
        .unwrap();
        assert_eq!(args.from, "LUX");
        assert_eq!(args.to, "JFK");
        assert!(args.trip_days.is_none());
        assert_eq!(args.months, 3);
    }

    #[test]
    fn cheap_args_parse_round_trip() {
        let args = CheapArgs::try_parse_from([
            "cheap",
            "--from",
            "LUX",
            "--to",
            "JFK",
            "--date",
            "2026-09-01",
            "--trip-days",
            "7",
            "--months",
            "6",
        ])
        .unwrap();
        assert_eq!(args.trip_days, Some(7));
        assert_eq!(args.months, 6);
    }

    #[test]
    fn cheap_args_requires_from() {
        assert!(
            CheapArgs::try_parse_from(["cheap", "--to", "JFK", "--date", "2026-09-01"]).is_err()
        );
    }
}
