//! CLI subcommand: `gflights deals`
//!
//! Finds discounted destinations from an origin using the Google Flights deals
//! endpoint (GetFlightDealsStreaming).

use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use gflights::parsers::common::{Location, PlaceType, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::deals::DealConfig;
use gflights::requests::config::Currency;

use super::OutputFormat;

/// Arguments for the `deals` subcommand.
#[derive(Parser, Debug)]
pub struct DealsArgs {
    /// Origin airport IATA code (e.g. LUX).
    #[arg(long)]
    pub from: String,

    /// Outbound date (YYYY-MM-DD). With --ret, defines the trip-length anchor.
    #[arg(long)]
    pub out: NaiveDate,

    /// Return date (YYYY-MM-DD).
    #[arg(long)]
    pub ret: NaiveDate,

    /// Only show non-stop deals.
    #[arg(long)]
    pub nonstop: bool,

    /// Maximum one-way flight duration in hours.
    #[arg(long)]
    pub max_hours: Option<u32>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    pub class: TravelClass,

    /// Currency for prices.
    #[arg(long, default_value = "euro")]
    pub currency: Currency,

    /// BCP-47 language subtag.
    #[arg(long, default_value = "en")]
    pub lang: String,

    /// ISO 3166-1 alpha-2 country code.
    #[arg(long, default_value = "GB")]
    pub country: String,

    /// Output format.
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
}

pub async fn cmd_deals(args: DealsArgs, client: &ApiClient) -> Result<()> {
    let origin = Location {
        loc_identifier: args.from.to_uppercase(),
        loc_type: PlaceType::Airport,
        location_name: None,
    };
    let travellers = Travelers::new(vec![args.adults as i32, 0, 0, 0])?;

    let config = DealConfig {
        origin: vec![origin],
        outbound_date: args.out,
        return_date: args.ret,
        nonstop: args.nonstop,
        max_duration_minutes: args.max_hours.map(|h| h * 60),
        travel_class: args.class,
        travellers,
        currency: args.currency,
        language: args.lang,
        country: args.country,
    };

    let mut deals = client.request_deals(&config).await?;
    // Best discounts first.
    deals.sort_by_key(|d| std::cmp::Reverse(d.discount_pct));

    if deals.is_empty() {
        eprintln!("No deals found.");
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&deals)?);
        }
        OutputFormat::Table => {
            println!(
                "{:>5}  {:>6}  {:>7}  {:<22}  {:<8}  {:>5}  {:>5}  {:<24}",
                "OFF%", "PRICE", "TYPICAL", "DESTINATION", "AIRLINE", "STOPS", "MINS", "DATES"
            );
            println!("{}", "-".repeat(100));
            for d in &deals {
                let off = d
                    .discount_pct
                    .map(|p| format!("{p}%"))
                    .unwrap_or_else(|| "—".into());
                let price = d.price.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
                let typical = d
                    .typical_price
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "—".into());
                let dest = format!("{} ({})", d.destination_city, d.destination_iata);
                let airline = d.airline_name.as_deref().unwrap_or("—");
                let stops = d.stops.map(|s| s.to_string()).unwrap_or_else(|| "—".into());
                let mins = d
                    .duration_minutes
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "—".into());
                let dates = match (d.outbound_date, d.return_date) {
                    (Some(o), Some(r)) => format!("{o} → {r}"),
                    (Some(o), None) => o.to_string(),
                    _ => "—".into(),
                };
                println!(
                    "{:>5}  {:>6}  {:>7}  {:<22}  {:<8}  {:>5}  {:>5}  {:<24}",
                    off, price, typical, dest, airline, stops, mins, dates
                );
                if let Some(url) = &d.booking_url {
                    println!("       {url}");
                }
            }
        }
    }
    Ok(())
}
