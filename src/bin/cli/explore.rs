//! CLI subcommand: `gflights explore`
//!
//! Searches for cheap destinations from a given origin airport using the
//! Google Flights Explore (GetExploreDestinations) endpoint.

use anyhow::Result;
use clap::{Parser, ValueEnum};
use gflights::parsers::common::{Location, PlaceType, TravelClass};
use gflights::requests::api::ApiClient;
use gflights::requests::config::explore::{ExploreConfig, ExploreDate, ExploreDuration, Interest};
use gflights::requests::config::Currency;

use super::OutputFormat;

// ---------------------------------------------------------------------------
// Duration value for clap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DurationArg {
    /// A weekend trip (Sat+Sun).
    Weekend,
    /// ~1 week.
    Week,
    /// ~2 weeks.
    #[value(name = "2weeks")]
    TwoWeeks,
}

impl From<DurationArg> for ExploreDuration {
    fn from(d: DurationArg) -> Self {
        match d {
            DurationArg::Weekend => ExploreDuration::Weekend,
            DurationArg::Week => ExploreDuration::OneWeek,
            DurationArg::TwoWeeks => ExploreDuration::TwoWeeks,
        }
    }
}

// ---------------------------------------------------------------------------
// Interest value for clap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InterestArg {
    Outdoors,
    Beaches,
    Museums,
    History,
    Skiing,
}

impl InterestArg {
    fn to_mid(self) -> &'static str {
        match self {
            InterestArg::Outdoors => Interest::OUTDOORS,
            InterestArg::Beaches => Interest::BEACHES,
            InterestArg::Museums => Interest::MUSEUMS,
            InterestArg::History => Interest::HISTORY,
            InterestArg::Skiing => Interest::SKIING,
        }
    }
}

// ---------------------------------------------------------------------------
// Args struct
// ---------------------------------------------------------------------------

/// Find cheap destinations from an origin airport using Google Flights Explore.
#[derive(Parser, Debug)]
pub struct ExploreArgs {
    /// Origin airport IATA code (e.g. LUX, LHR).
    #[arg(long)]
    pub from: String,

    /// Calendar month to search in (1–12).  Omit for any month.
    #[arg(long)]
    pub month: Option<u8>,

    /// Trip duration.
    #[arg(long, default_value = "week")]
    pub duration: DurationArg,

    /// Maximum total round-trip budget.
    #[arg(long)]
    pub budget: Option<i32>,

    /// Interest category to filter destinations.
    #[arg(long)]
    pub interest: Option<InterestArg>,

    /// Maximum one-way flight time in hours.
    #[arg(long = "max-flight-hours")]
    pub max_flight_hours: Option<u32>,

    /// Number of carry-on bags.
    #[arg(long = "carry-on")]
    pub carry_on: Option<u8>,

    /// Number of checked bags.
    #[arg(long = "checked")]
    pub checked: Option<u8>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Cabin class.
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

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

pub async fn cmd_explore(args: ExploreArgs, client: &ApiClient) -> Result<()> {
    // Resolve origin IATA to a Location.  We skip the city-lookup API since
    // the explore endpoint accepts raw IATA codes directly.
    let origin = Location {
        loc_identifier: args.from.to_uppercase(),
        loc_type: PlaceType::Airport,
        location_name: None,
    };

    let travelers = gflights::parsers::common::Travelers::new(vec![args.adults as i32, 0, 0, 0])?;

    let trip_date = args.month.map(|m| ExploreDate { month: m });

    let max_flight_duration_minutes = args.max_flight_hours.map(|h| h * 60);

    let baggage = match (args.carry_on, args.checked) {
        (None, None) => None,
        (carry_on, checked) => Some((carry_on.unwrap_or(0), checked.unwrap_or(0))),
    };

    let interest = args.interest.map(|i| i.to_mid().to_string());

    let config = ExploreConfig {
        origin: vec![origin],
        trip_date,
        trip_duration: args.duration.into(),
        max_price: args.budget,
        interest,
        airline_alliance: None,
        max_flight_duration_minutes,
        baggage,
        map_bounds: None,
        travellers: travelers,
        travel_class: args.class,
        currency: args.currency,
        language: args.lang,
        country: args.country,
    };

    let results = client.request_explore(&config).await?;

    if results.is_empty() {
        eprintln!("No destinations found.");
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        OutputFormat::Table => {
            println!(
                "{:<20}  {:<12}  {:<5}  {:>6}  {:<8}  {:>5}  DATES",
                "DESTINATION", "COUNTRY", "ARPT", "PRICE", "AIRLINE", "STOPS"
            );
            println!("{}", "-".repeat(80));
            for r in &results {
                let price_str = r.price.map(|p| p.to_string()).unwrap_or_else(|| "—".into());
                let airline_str = r.airline.as_deref().unwrap_or("—");
                let stops_str = r.stops.map(|s| s.to_string()).unwrap_or_else(|| "—".into());
                let dates_str = match (r.date_from, r.date_to) {
                    (Some(f), Some(t)) => format!("{f} → {t}"),
                    (Some(f), None) => f.to_string(),
                    _ => "—".to_string(),
                };
                println!(
                    "{:<20}  {:<12}  {:<5}  {:>6}  {:<8}  {:>5}  {}",
                    truncate(&r.name, 20),
                    truncate(&r.country, 12),
                    r.nearest_airport,
                    price_str,
                    airline_str,
                    stops_str,
                    dates_str,
                );
            }
        }
    }

    Ok(())
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((i, _)) => &s[..i],
    }
}
