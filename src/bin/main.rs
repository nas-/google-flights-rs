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
//! gflights> quit
//! ```

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use gflights::parsers::common::{SortOrder, StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Config, Currency};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

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
// Top-level CLI  (optional subcommand → REPL when absent)
// ---------------------------------------------------------------------------

/// Unofficial Google Flights command-line client.
///
/// Run without arguments to enter the interactive REPL.
#[derive(Parser, Debug)]
#[command(name = "gflights", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Wrapper used inside the REPL to parse a single line as a subcommand.
#[derive(Parser, Debug)]
#[command(name = "gflights", disable_help_flag = true)]
struct ReplCommand {
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
    /// Exit the interactive REPL (alias: exit).
    #[command(alias = "exit")]
    Quit,
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

    // Create one shared ApiClient (fetches the frontend version once).
    let client = ApiClient::new().await;

    match cli.command {
        Some(cmd) => run_command(cmd, &client).await,
        None => run_repl(&client).await,
    }
}

// ---------------------------------------------------------------------------
// Dispatch a parsed command
// ---------------------------------------------------------------------------

async fn run_command(cmd: Commands, client: &ApiClient) -> Result<()> {
    match cmd {
        Commands::Search(args) => cmd_search(args, client).await,
        Commands::Graph(args) => cmd_graph(args, client).await,
        Commands::Offer(args) => cmd_offer(args, client).await,
        Commands::Quit => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Interactive REPL
// ---------------------------------------------------------------------------

async fn run_repl(client: &ApiClient) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    println!("gflights interactive mode  (type 'help' for usage, 'quit' to exit)");

    loop {
        match rl.readline("gflights> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);

                if line == "help" || line == "--help" || line == "-h" {
                    println!("Commands:");
                    println!("  search --from <CODE> --to <CODE> --date <YYYY-MM-DD> [OPTIONS]");
                    println!("  graph  --from <CODE> --to <CODE> --date <YYYY-MM-DD> [--months N]");
                    println!("  offer  --from <CODE> --to <CODE> --date <YYYY-MM-DD> [OPTIONS]");
                    println!("  quit / exit");
                    continue;
                }

                // Prepend program name so clap can parse "search --from …" correctly.
                let parts: Vec<String> = std::iter::once("gflights".to_string())
                    .chain(line.split_whitespace().map(String::from))
                    .collect();

                match ReplCommand::try_parse_from(&parts) {
                    Ok(rc) => {
                        if matches!(rc.command, Commands::Quit) {
                            break;
                        }
                        if let Err(e) = run_command(rc.command, client).await {
                            eprintln!("Error: {e:#}");
                        }
                    }
                    Err(e) => eprintln!("{e}"),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Readline error: {e}");
                break;
            }
        }
    }
    Ok(())
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

    builder.build()
}

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

async fn cmd_search(args: SearchArgs, client: &ApiClient) -> Result<()> {
    let config = build_config(&args.common, client)
        .await?
        .with_sort_order(args.sort);

    let results = client.request_flights(&config).await?;
    let flights = results.get_all_flights();

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
                "{:<8}  {:>6}  {:>5}  {:>5}  ROUTE",
                "AIRLINE", "PRICE", "STOPS", "MINS"
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

async fn cmd_graph(args: GraphArgs, client: &ApiClient) -> Result<()> {
    use chrono::Months;

    let config = build_config(&args.common, client).await?;
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
            println!("{:<12}  {:>8}", "DATE", "PRICE");
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

/// Maximum display width for booking URLs in the table view.
const URL_DISPLAY_MAX: usize = 60;

fn truncate_url(url: &str) -> String {
    if url.len() <= URL_DISPLAY_MAX {
        url.to_string()
    } else {
        format!("{}…", &url[..URL_DISPLAY_MAX])
    }
}

async fn cmd_offer(args: OfferArgs, client: &ApiClient) -> Result<()> {
    let config = build_config(&args.common, client).await?;

    let result = client.request_flights(&config).await?;
    let first_flight = result.get_all_flights().into_iter().next();

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
        if let Some(ret) = second_result.get_all_flights().into_iter().next() {
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
            println!("{:<30}  {:>8}  URL", "AIRLINE(S)", "PRICE");
            println!("{}", "-".repeat(80));
            for o in &groups {
                let airlines = o.airline_names.join(", ");
                let price = o.price.unwrap_or(0);
                // Resolve booking URL if a click token is available.
                let url = if let Some(token) = o.click_token.as_deref() {
                    match client.resolve_booking_url(token).await {
                        Ok(u) => truncate_url(&u),
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
