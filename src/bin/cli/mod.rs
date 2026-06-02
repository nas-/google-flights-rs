use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use gflights::parsers::common::{StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Config, Currency};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub mod date_grid;
pub mod graph;
pub mod multi_city;
pub mod offer;
pub mod search;

use date_grid::{cmd_date_grid, DateGridArgs};
use graph::{cmd_graph, GraphArgs};
use multi_city::{cmd_multi_city, MultiCityArgs};
use offer::{cmd_offer, OfferArgs};
use search::{cmd_search, SearchArgs};

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
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
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Wrapper used inside the REPL to parse a single line as a subcommand.
#[derive(Parser, Debug)]
#[command(name = "gflights", disable_help_flag = true)]
pub struct ReplCommand {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search for available flights.
    Search(SearchArgs),
    /// Show cheapest prices across a date range (price graph).
    Graph(GraphArgs),
    /// Show a price grid across departure × return date windows (round trips only).
    #[command(name = "dgrid")]
    DateGrid(DateGridArgs),
    /// Show booking offers (with airline prices and URLs) for a specific itinerary.
    Offer(OfferArgs),
    /// Multi-city (open-jaw) flight search across 2+ legs.
    ///
    /// Specify each leg with --leg FROM,TO,DATE (repeatable).
    ///
    /// Example: gflights mcity --leg LUX,FCO,2026-09-10 --leg FCO,MAD,2026-09-13 --leg MAD,LUX,2026-09-17
    #[command(name = "mcity")]
    MultiCity(MultiCityArgs),
    /// Exit the interactive REPL (alias: exit).
    #[command(alias = "exit")]
    Quit,
}

// ---------------------------------------------------------------------------
// Shared options
// ---------------------------------------------------------------------------

/// Options shared by all subcommands that use a single date-pair route.
#[derive(Parser, Debug)]
pub struct CommonArgs {
    /// Departure airport IATA code or city name (e.g. LHR, "London").
    #[arg(long)]
    pub from: String,

    /// Destination airport IATA code or city name (e.g. JFK, "New York").
    #[arg(long)]
    pub to: String,

    /// Outbound departure date in YYYY-MM-DD format.
    #[arg(long)]
    pub date: NaiveDate,

    /// Return date in YYYY-MM-DD format (omit for one-way).
    #[arg(long)]
    pub r#return: Option<NaiveDate>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    pub class: TravelClass,

    /// Stop filter.
    #[arg(long, default_value = "all")]
    pub stops: StopOptions,

    /// Currency for prices.
    #[arg(long, default_value = "euro")]
    pub currency: Currency,

    /// BCP-47 language subtag (e.g. en, fr, de).
    #[arg(long, default_value = "en")]
    pub lang: String,

    /// ISO 3166-1 alpha-2 country code (e.g. GB, FR, US).
    #[arg(long, default_value = "GB")]
    pub country: String,

    /// Output format.
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
}

// ---------------------------------------------------------------------------
// Build Config from CommonArgs (shared helper)
// ---------------------------------------------------------------------------

pub async fn build_config(common: &CommonArgs, client: &ApiClient) -> Result<Config> {
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
// Dispatch a parsed command
// ---------------------------------------------------------------------------

pub async fn run_command(cmd: Commands, client: &ApiClient) -> Result<()> {
    match cmd {
        Commands::Search(args) => cmd_search(args, client).await,
        Commands::Graph(args) => cmd_graph(args, client).await,
        Commands::DateGrid(args) => cmd_date_grid(args, client).await,
        Commands::Offer(args) => cmd_offer(args, client).await,
        Commands::MultiCity(args) => cmd_multi_city(args, client).await,
        Commands::Quit => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Interactive REPL
// ---------------------------------------------------------------------------

pub async fn run_repl(client: &ApiClient) -> Result<()> {
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
                    println!("  mcity  --leg FROM,TO,DATE [--leg FROM,TO,DATE ...] [OPTIONS]");
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
                    // For all clap errors (missing required args, --help, --version,
                    // unknown flags, …) use clap's own formatter so the user sees
                    // coloured output with a "Usage:" hint instead of a raw error
                    // string.  We never call process::exit here so the REPL continues.
                    Err(e) => {
                        e.print().unwrap_or_default();
                    }
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
        let e = result.unwrap_err();
        assert!(
            matches!(e.kind(), ErrorKind::MissingRequiredArgument),
            "expected MissingRequiredArgument, got: {:?}",
            e.kind()
        );
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
    fn repl_parse_mcity_two_legs() {
        let rc = parse(&[
            "mcity",
            "--leg",
            "LUX,FCO,2026-09-10",
            "--leg",
            "FCO,MAD,2026-09-13",
        ])
        .expect("mcity with 2 legs should parse");
        match rc.command {
            Commands::MultiCity(args) => {
                assert_eq!(args.legs.len(), 2);
                assert_eq!(args.legs[0].from, "LUX");
                assert_eq!(args.legs[0].to, "FCO");
                assert_eq!(args.legs[1].from, "FCO");
            }
            other => panic!("expected MultiCity, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_mcity_three_legs() {
        let rc = parse(&[
            "mcity",
            "--leg",
            "LUX,FCO,2026-09-10",
            "--leg",
            "FCO,MAD,2026-09-13",
            "--leg",
            "MAD,LUX,2026-09-17",
        ])
        .expect("mcity with 3 legs should parse");
        match rc.command {
            Commands::MultiCity(args) => assert_eq!(args.legs.len(), 3),
            other => panic!("expected MultiCity, got {other:?}"),
        }
    }

    #[test]
    fn repl_parse_mcity_invalid_leg_format() {
        let result = parse(&["mcity", "--leg", "LUX-FCO-2026-09-10"]);
        assert!(result.is_err(), "invalid leg format should error");
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
