//! `gflights` — command-line interface for the gflights library.
//!
//! # Subcommands
//!
//! ```text
//! gflights search --from LHR --to JFK --date 2025-08-01 [OPTIONS]
//! gflights graph  --from LHR --to JFK --date 2025-08-01 [--months 3] [OPTIONS]
//! gflights offer  --from LHR --to JFK --date 2025-08-01 [OPTIONS]
//! ```

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use gflights::parsers::common::{SortOrder, StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Config, Currency};

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    /// Human-readable aligned table (default).
    #[default]
    Table,
    /// JSON — suitable for piping to `jq` or other tools.
    Json,
}

// ---------------------------------------------------------------------------
// Top-level CLI
// ---------------------------------------------------------------------------

/// Unofficial Google Flights command-line client.
#[derive(Parser, Debug)]
#[command(name = "gflights", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Search for available flights.
    Search(SearchArgs),
    /// Show cheapest prices across a date range (price graph).
    Graph(GraphArgs),
    /// Show booking offers (with airline prices and URLs) for a specific itinerary.
    Offer(OfferArgs),
}

// ---------------------------------------------------------------------------
// Shared options
// ---------------------------------------------------------------------------

/// Options shared by all three subcommands.
#[derive(Parser, Debug)]
struct CommonArgs {
    /// Departure airport IATA code or city name (e.g. LHR, "London").
    #[arg(long)]
    from: String,

    /// Destination airport IATA code or city name (e.g. JFK, "New York").
    #[arg(long)]
    to: String,

    /// Outbound departure date in YYYY-MM-DD format.
    #[arg(long)]
    date: NaiveDate,

    /// Return date in YYYY-MM-DD format (omit for one-way).
    #[arg(long)]
    r#return: Option<NaiveDate>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    class: TravelClass,

    /// Stop filter.
    #[arg(long, default_value = "all")]
    stops: StopOptions,

    /// Currency for prices.
    #[arg(long, default_value = "euro")]
    currency: Currency,

    /// BCP-47 language subtag (e.g. en, fr, de).
    #[arg(long, default_value = "en")]
    lang: String,

    /// ISO 3166-1 alpha-2 country code (e.g. GB, FR, US).
    #[arg(long, default_value = "GB")]
    country: String,

    /// Output format.
    #[arg(long, default_value = "table")]
    format: OutputFormat,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct SearchArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Sort order.
    #[arg(long, default_value = "best")]
    sort: SortOrder,
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct GraphArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Number of months to show in the price graph.
    #[arg(long, default_value = "3")]
    months: u32,
}

// ---------------------------------------------------------------------------
// Offer
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct OfferArgs {
    #[command(flatten)]
    common: CommonArgs,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

    match cli.command {
        Commands::Search(args) => cmd_search(args).await,
        Commands::Graph(args) => cmd_graph(args).await,
        Commands::Offer(args) => cmd_offer(args).await,
    }
}

// ---------------------------------------------------------------------------
// Build Config from CommonArgs (shared helper)
// ---------------------------------------------------------------------------

async fn build_config(common: &CommonArgs, client: &ApiClient) -> Result<Config> {
    let travelers = Travelers::new(vec![common.adults as i32, 0, 0, 0])?;

    let mut builder = Config::builder()
        .departure(&common.from, client)
        .await?
        .destination(&common.to, client)
        .await?
        .departing_date(common.date)
        .travelers(travelers)
        .travel_class(common.class)
        .stop_options(common.stops)
        .currency(common.currency.clone())
        .language(common.lang.clone())
        .country(common.country.clone());

    if let Some(ret) = common.r#return {
        builder = builder.return_date(ret);
    }

    Ok(builder.build()?)
}

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

async fn cmd_search(args: SearchArgs) -> Result<()> {
    let client = ApiClient::new().await;
    let config = build_config(&args.common, &client)
        .await?
        .with_sort_order(args.sort);

    let results = client.request_flights(&config).await?;

    let flights: Vec<_> = results
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .collect();

    if flights.is_empty() {
        eprintln!("No flights found.");
        return Ok(());
    }

    match args.common.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&flights)?);
        }
        OutputFormat::Table => {
            println!(
                "{:<8}  {:>6}  {:>5}  {:>5}  {}",
                "AIRLINE", "PRICE", "STOPS", "MINS", "ROUTE"
            );
            println!("{}", "-".repeat(60));
            for f in &flights {
                let price = f
                    .itinerary_cost
                    .trip_cost
                    .as_ref()
                    .map(|c| c.price.to_string())
                    .unwrap_or_else(|| "—".into());
                let from = f
                    .itinerary
                    .flight_details
                    .first()
                    .map(|d| d.departure_airport_code.as_str())
                    .unwrap_or("?");
                let to = f
                    .itinerary
                    .flight_details
                    .last()
                    .map(|d| d.destination_airport_code.as_str())
                    .unwrap_or("?");
                println!(
                    "{:<8}  {:>6}  {:>5}  {:>5}  {}→{}",
                    f.itinerary.flight_by,
                    price,
                    f.itinerary.stop_count(),
                    f.itinerary.total_time_minutes,
                    from,
                    to,
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// graph
// ---------------------------------------------------------------------------

async fn cmd_graph(args: GraphArgs) -> Result<()> {
    use chrono::Months;

    let client = ApiClient::new().await;
    let config = build_config(&args.common, &client).await?;
    let months = Months::new(args.months);

    let graph = client.request_graph(&config, months).await?;
    let mut points: Vec<_> = graph
        .get_all_graphs()
        .into_iter()
        .filter_map(|g| g.maybe_get_date_price())
        .collect();
    points.sort_by_key(|&(date, _)| date);

    if points.is_empty() {
        eprintln!("No price data found.");
        return Ok(());
    }

    match args.common.format {
        OutputFormat::Json => {
            let json: Vec<_> = points
                .iter()
                .map(|(d, p)| serde_json::json!({"date": d.to_string(), "price": p}))
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Table => {
            let min_price = points.iter().map(|&(_, p)| p).min().unwrap_or(0);
            println!("{:<12}  {:>8}  {}", "DATE", "PRICE", "");
            println!("{}", "-".repeat(50));
            for (date, price) in &points {
                let marker = if *price == min_price {
                    " ← cheapest"
                } else {
                    ""
                };
                println!("{:<12}  {:>8}{}", date, price, marker);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// offer
// ---------------------------------------------------------------------------

async fn cmd_offer(args: OfferArgs) -> Result<()> {
    let client = ApiClient::new().await;
    let config = build_config(&args.common, &client).await?;

    let result = client.request_flights(&config).await?;
    let first_flight = result
        .responses
        .iter()
        .filter_map(|r| r.maybe_get_all_flights())
        .flatten()
        .next();

    let first = match first_flight {
        Some(f) => f,
        None => {
            eprintln!("No flights found to price.");
            return Ok(());
        }
    };

    config.fixed_flights.add_element(first)?;

    // If return trip, also lock in the cheapest return leg.
    if config.return_date.is_some() {
        let second_result = client.request_flights(&config).await?;
        if let Some(ret) = second_result
            .responses
            .iter()
            .filter_map(|r| r.maybe_get_all_flights())
            .flatten()
            .next()
        {
            config.fixed_flights.add_element(ret)?;
        }
    }

    let offers = client.request_offer(&config).await?;

    let mut groups: Vec<_> = offers
        .response
        .iter()
        .flat_map(|r| &r.offers)
        .filter(|o| o.price.is_some())
        .collect();
    groups.sort_by_key(|o| o.price.unwrap_or(i32::MAX));

    if groups.is_empty() {
        eprintln!("No offers found.");
        return Ok(());
    }

    match args.common.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&offers.response)?);
        }
        OutputFormat::Table => {
            println!("{:<30}  {:>8}  {}", "AIRLINE(S)", "PRICE", "URL");
            println!("{}", "-".repeat(80));
            for o in &groups {
                let airlines = o.airline_names.join(", ");
                let price = o.price.unwrap_or(0);
                // Resolve booking URL if a click token is available.
                let url = if let Some(token) = o.click_token.as_deref() {
                    match client.resolve_booking_url(token).await {
                        Ok(u) => u,
                        Err(_) => "(URL unavailable)".into(),
                    }
                } else {
                    "(no token)".into()
                };
                println!("{:<30}  {:>8}  {}", airlines, price, url);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Extension trait to apply sort order post-build
// ---------------------------------------------------------------------------

trait WithSortOrder {
    fn with_sort_order(self, sort: SortOrder) -> Self;
}

impl WithSortOrder for Config {
    fn with_sort_order(mut self, sort: SortOrder) -> Self {
        self.sort_order = sort;
        self
    }
}
