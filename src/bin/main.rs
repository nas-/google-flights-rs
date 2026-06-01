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

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use gflights::parsers::common::{
    AirlineFilter, SortOrder, StopOptions, StopoverDuration, TravelClass, Travelers,
};
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
    /// Show a price grid across departure × return date windows (round trips only).
    #[command(name = "dgrid")]
    DateGrid(DateGridArgs),
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

    /// Minimum layover duration in minutes (rounded up to the next 30 min interval).
    #[arg(long)]
    min_layover: Option<u32>,

    /// Maximum layover duration in minutes (rounded up to the next 30 min interval).
    #[arg(long)]
    max_layover: Option<u32>,

    /// Restrict results to lower-CO₂ emissions flights.
    #[arg(long)]
    lower_emissions: bool,

    /// Airline IATA code (e.g. LX, LH) or alliance name (ONEWORLD, SKYTEAM, STAR_ALLIANCE)
    /// to include. May be repeated for multiple airlines/alliances.
    #[arg(long = "airline")]
    airlines: Vec<AirlineFilter>,

    /// Airline IATA code or alliance name to exclude.
    /// May be repeated for multiple airlines/alliances.
    #[arg(long = "exclude-airline")]
    exclude_airlines: Vec<AirlineFilter>,

    /// Require a connection through this IATA airport code (e.g. CDG, AMS).
    /// May be repeated for multiple airports.
    #[arg(long = "via")]
    connecting_airports: Vec<String>,
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
// Date grid
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct DateGridArgs {
    /// Departure airport IATA code or city name.
    #[arg(long)]
    from: String,

    /// Destination airport IATA code or city name.
    #[arg(long)]
    to: String,

    /// First day of the outbound departure window (YYYY-MM-DD).
    #[arg(long)]
    dep_start: NaiveDate,

    /// Last day of the outbound departure window (YYYY-MM-DD).
    #[arg(long)]
    dep_end: NaiveDate,

    /// First day of the return window (YYYY-MM-DD).
    #[arg(long)]
    ret_start: NaiveDate,

    /// Last day of the return window (YYYY-MM-DD).
    #[arg(long)]
    ret_end: NaiveDate,

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

    /// Output format.
    #[arg(long, default_value = "table")]
    format: OutputFormat,
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
        Commands::DateGrid(args) => cmd_date_grid(args, client).await,
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
                    println!("  dgrid  --from <CODE> --to <CODE> --dep-start <DATE> --dep-end <DATE> --ret-start <DATE> --ret-end <DATE>");
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
                    // clap would normally call process::exit for --help / --version;
                    // intercept those kinds and just print without exiting the REPL.
                    Err(e)
                        if matches!(
                            e.kind(),
                            clap::error::ErrorKind::DisplayHelp
                                | clap::error::ErrorKind::DisplayVersion
                        ) =>
                    {
                        print!("{e}");
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
    let mut config = build_config(&args.common, client)
        .await?
        .with_sort_order(args.sort);

    // Apply filter flags that live on SearchArgs rather than CommonArgs.
    config.airlines_include = args.airlines;
    config.airlines_exclude = args.exclude_airlines;
    config.connecting_airports = args.connecting_airports;
    config.lower_emissions = args.lower_emissions;
    if let Some(mins) = args.min_layover {
        config.stopover_min = StopoverDuration::Minutes(mins);
    }
    if let Some(mins) = args.max_layover {
        config.stopover_max = StopoverDuration::Minutes(mins);
    }

    let results = client.request_flights(&config).await?;
    let mut flights = results.get_all_flights();

    // Client-side sort — guarantees the requested order regardless of what
    // Google returns.  `Best` keeps Google's own ordering.
    match args.sort {
        SortOrder::Best => {}
        SortOrder::Price => {
            flights.sort_by_key(|f| {
                f.itinerary_cost
                    .trip_cost
                    .as_ref()
                    .map(|c| c.price)
                    .unwrap_or(i32::MAX)
            });
        }
        SortOrder::Duration => {
            flights.sort_by_key(|f| f.itinerary.total_time_minutes);
        }
        SortOrder::DepartureTime => {
            flights.sort_by_key(|f| {
                f.itinerary
                    .flight_details
                    .first()
                    .map(|d| d.departure_time.hour.unwrap_or(0) * 60 + d.departure_time.minute)
            });
        }
        SortOrder::ArrivalTime => {
            flights.sort_by_key(|f| {
                f.itinerary
                    .flight_details
                    .last()
                    .map(|d| d.arrival_time.hour.unwrap_or(0) * 60 + d.arrival_time.minute)
            });
        }
    }

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
// dgrid
// ---------------------------------------------------------------------------

async fn cmd_date_grid(args: DateGridArgs, client: &ApiClient) -> Result<()> {
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
        .currency(args.currency.clone())
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

// ---------------------------------------------------------------------------
// offer
// ---------------------------------------------------------------------------

/// Render a terminal hyperlink (OSC 8) when stdout is a TTY; plain URL otherwise.
///
/// Supported by: Windows Terminal, iTerm2, GNOME Terminal, VTE-based terminals.
/// In a non-TTY context (pipe / file redirect) the raw URL is emitted instead.
fn render_link(url: &str, label: &str) -> String {
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() {
        // OSC 8 ; params ; uri ST  label  OSC 8 ; ; ST
        format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
    } else {
        url.to_string()
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
                let link = if let Some(token) = o.click_token.as_deref() {
                    match client.resolve_booking_url(token).await {
                        Ok(u) => render_link(&u, "Book"),
                        Err(_) => "(URL unavailable)".into(),
                    }
                } else {
                    "(no token)".into()
                };
                println!("{:<30}  {:>8}  {}", airlines, price, link);
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

// ---------------------------------------------------------------------------
// Tests for REPL command parsing (no network, pure clap)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<ReplCommand, clap::Error> {
        let parts: Vec<String> = std::iter::once("gflights")
            .chain(args.iter().copied())
            .map(String::from)
            .collect();
        ReplCommand::try_parse_from(&parts)
    }

    #[test]
    fn repl_parse_quit_command() {
        let rc = parse(&["quit"]).expect("quit should parse");
        assert!(matches!(rc.command, Commands::Quit));
    }

    #[test]
    fn repl_parse_exit_alias() {
        let rc = parse(&["exit"]).expect("exit should parse");
        assert!(matches!(rc.command, Commands::Quit));
    }

    #[test]
    fn repl_parse_search_minimal() {
        let rc = parse(&[
            "search",
            "--from",
            "LHR",
            "--to",
            "JFK",
            "--date",
            "2026-08-01",
        ])
        .expect("minimal search should parse");
        match rc.command {
            Commands::Search(args) => {
                assert_eq!(args.common.from, "LHR");
                assert_eq!(args.common.to, "JFK");
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_search_with_return() {
        let rc = parse(&[
            "search",
            "--from",
            "MXP",
            "--to",
            "SVO",
            "--date",
            "2026-08-01",
            "--return",
            "2026-08-15",
        ])
        .expect("search with return should parse");
        match rc.command {
            Commands::Search(args) => {
                assert!(args.common.r#return.is_some());
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_dgrid_command() {
        let rc = parse(&[
            "dgrid",
            "--from",
            "LHR",
            "--to",
            "JFK",
            "--dep-start",
            "2026-08-01",
            "--dep-end",
            "2026-08-07",
            "--ret-start",
            "2026-08-15",
            "--ret-end",
            "2026-08-22",
        ])
        .expect("dgrid should parse");
        assert!(matches!(rc.command, Commands::DateGrid(_)));
    }

    #[test]
    fn repl_parse_graph_with_months() {
        let rc = parse(&[
            "graph",
            "--from",
            "SVO",
            "--to",
            "CDG",
            "--date",
            "2026-09-01",
            "--months",
            "6",
        ])
        .expect("graph with months should parse");
        match rc.command {
            Commands::Graph(args) => assert_eq!(args.months, 6),
            other => panic!("expected Graph, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_offer_command() {
        let rc = parse(&[
            "offer",
            "--from",
            "FRA",
            "--to",
            "NRT",
            "--date",
            "2026-09-01",
        ])
        .expect("offer should parse");
        assert!(matches!(rc.command, Commands::Offer(_)));
    }

    #[test]
    fn repl_parse_invalid_command_returns_error() {
        let result = parse(&["bogus"]);
        assert!(result.is_err(), "unknown subcommand should error");
    }

    #[test]
    fn repl_parse_missing_required_returns_error() {
        // search without any arguments should error (all three of --from/--to/--date required)
        let result = parse(&["search"]);
        assert!(result.is_err(), "search without required args should error");
    }

    #[test]
    fn repl_parse_help_flag_always_errors() {
        // `disable_help_flag = true` on ReplCommand propagates to sub-parsers, so
        // `--help` triggers UnknownArgument rather than DisplayHelp.  Either way
        // the REPL handles it gracefully (prints and continues, no process::exit).
        let result = parse(&["search", "--help"]);
        assert!(result.is_err(), "--help in REPL context must always error");
        let e = result.unwrap_err();
        assert!(
            matches!(
                e.kind(),
                ErrorKind::DisplayHelp | ErrorKind::UnknownArgument
            ),
            "expected DisplayHelp or UnknownArgument, got: {:?}",
            e.kind()
        );
    }

    #[test]
    fn repl_parse_search_filter_flags() {
        let rc = parse(&[
            "search",
            "--from",
            "LHR",
            "--to",
            "JFK",
            "--date",
            "2026-08-01",
            "--min-layover",
            "60",
            "--max-layover",
            "180",
            "--lower-emissions",
            "--airline",
            "LX",
            "--airline",
            "ONEWORLD",
            "--exclude-airline",
            "FR",
            "--via",
            "CDG",
        ])
        .expect("search with filter flags should parse");
        match rc.command {
            Commands::Search(args) => {
                assert_eq!(args.min_layover, Some(60));
                assert_eq!(args.max_layover, Some(180));
                assert!(args.lower_emissions);
                assert_eq!(args.airlines.len(), 2);
                assert_eq!(args.exclude_airlines.len(), 1);
                assert_eq!(args.connecting_airports, vec!["CDG"]);
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_invalid_date_returns_error() {
        let result = parse(&[
            "search",
            "--from",
            "LHR",
            "--to",
            "JFK",
            "--date",
            "not-a-date",
        ]);
        assert!(result.is_err(), "invalid date should error");
    }

    #[test]
    fn repl_parse_search_accepts_sort_order_values() {
        for sort in &["best", "price", "duration"] {
            let rc = parse(&[
                "search",
                "--from",
                "LHR",
                "--to",
                "JFK",
                "--date",
                "2026-08-01",
                "--sort",
                sort,
            ])
            .unwrap_or_else(|e| panic!("sort={sort} should parse: {e}"));
            assert!(matches!(rc.command, Commands::Search(_)));
        }
    }
}
