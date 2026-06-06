use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use gflights::parsers::common::{StopOptions, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Config, Currency};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub mod cheap;
pub mod date_grid;
pub mod deals;
pub mod explore;
pub mod graph;
pub mod mcp;
pub mod multi_city;
pub mod offer;
pub mod search;
pub mod select;

use cheap::{cmd_cheap, CheapArgs};
use date_grid::{cmd_date_grid, DateGridArgs};
use deals::{cmd_deals, DealsArgs};
use explore::{cmd_explore, ExploreArgs};
use graph::{cmd_graph, GraphArgs};
use mcp::run_mcp;
use multi_city::{cmd_multi_city, MultiCityArgs};
use offer::{cmd_offer, OfferArgs};
use search::{cmd_search, SearchArgs};
use select::{cmd_select, SelectArgs};

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
    /// Override the User-Agent header (default: a random real desktop browser
    /// string chosen per run).
    #[arg(long, global = true)]
    pub user_agent: Option<String>,

    /// Route all requests through a proxy (e.g. http://host:3128,
    /// socks5://127.0.0.1:9050). Supports http(s) and socks5.
    #[arg(long, global = true)]
    pub proxy: Option<String>,

    /// Result currency for prices, applied to every request (e.g. euro,
    /// us-dollar, british-pound).
    #[arg(long, global = true, default_value = "euro")]
    pub currency: Currency,

    /// BCP-47 language subtag applied to every request (e.g. en, fr, de).
    #[arg(long, global = true, default_value = "en")]
    pub lang: String,

    /// ISO 3166-1 alpha-2 country code applied to every request (e.g. GB, FR, US).
    #[arg(long, global = true, default_value = "GB")]
    pub country: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// gflights interactive REPL.  Type 'help' for usage, 'quit' to exit.
#[derive(Parser, Debug)]
#[command(name = "gflights")]
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
    /// Interactively pick outbound (and return) flights, then a booking offer.
    ///
    /// Numbered prompts let you choose each leg, then an offer; prints the
    /// resolved booking URL. Works one-shot or inside the REPL.
    Select(SelectArgs),
    /// Multi-city (open-jaw) flight search across 2+ legs.
    ///
    /// Specify each leg with --leg FROM,TO,DATE (repeatable).
    ///
    /// Example: gflights mcity --leg LUX,FCO,2026-09-10 --leg FCO,MAD,2026-09-13 --leg MAD,LUX,2026-09-17
    #[command(name = "mcity")]
    MultiCity(MultiCityArgs),
    /// Find the cheapest departure dates for a route over a range of months.
    ///
    /// Use --trip-days N to search for round trips of exactly N nights.
    /// Omit --trip-days for one-way date discovery.
    #[command(name = "cheap")]
    Cheap(CheapArgs),
    /// Explore cheap destinations from an origin airport (Google Flights Explore).
    ///
    /// Example: gflights explore --from LUX --month 7 --duration week --budget 300
    #[command(name = "explore")]
    Explore(ExploreArgs),
    /// Find discounted destinations from an origin (Google Flights deals).
    ///
    /// Example: gflights deals --from LUX --out 2026-06-20 --ret 2026-06-24 --nonstop
    #[command(name = "deals")]
    Deals(DealsArgs),
    /// Run as an MCP (Model Context Protocol) server over stdio.
    ///
    /// Exposes flight tools (search, price_graph, cheapest_dates, explore,
    /// deals) to MCP clients such as Claude Desktop. Speaks JSON-RPC 2.0 on
    /// stdin/stdout.
    #[command(name = "mcp")]
    Mcp,
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

    /// Number of children (2–11 years).
    #[arg(long, default_value = "0")]
    pub children: u32,

    /// Number of infants in their own seat.
    #[arg(long = "infants-seat", default_value = "0")]
    pub infants_seat: u32,

    /// Number of lap infants.
    #[arg(long = "infants-lap", default_value = "0")]
    pub infants_lap: u32,

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

// ---------------------------------------------------------------------------
// Build Config from CommonArgs (shared helper)
// ---------------------------------------------------------------------------

pub async fn build_config(common: &CommonArgs, client: &ApiClient) -> Result<Config> {
    let travelers = Travelers::new(vec![
        common.adults as i32,
        common.children as i32,
        common.infants_lap as i32,
        common.infants_seat as i32,
    ])?;

    let mut builder = Config::builder()
        .departure(&common.from, client)
        .await?
        .destination(&common.to, client)
        .await?
        .departing_date(common.date)
        .travelers(travelers)
        .travel_class(common.class)
        .stop_options(common.stops);

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
        Commands::Select(args) => cmd_select(args, client).await,
        Commands::MultiCity(args) => cmd_multi_city(args, client).await,
        Commands::Cheap(args) => cmd_cheap(args, client).await,
        Commands::Explore(args) => cmd_explore(args, client).await,
        Commands::Deals(args) => cmd_deals(args, client).await,
        Commands::Mcp => run_mcp(client).await,
        Commands::Quit => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Interactive REPL — help text
// ---------------------------------------------------------------------------

fn print_repl_help() {
    println!(
        "\
Commands:

  search --from <CODE> --to <CODE> --date <YYYY-MM-DD> [--return <DATE>]
    Filters:  --stops all|nonstop|one-stop
              --airline <CODE|ALLIANCE>  (repeatable; e.g. --airline LX --airline ONEWORLD)
              --exclude-airline <CODE>   (repeatable)
              --via <CODE>               (require connection through this airport)
              --min-layover <MINS>       --max-layover <MINS>
              --lower-emissions          (restrict to below-average CO₂ flights)
    Output:   --sort best|price|duration|departure|arrival
              --show-co2                 (add CO₂ kg column to table)
              --detail                   (show layover airports; +1 for next-day arrivals)
              --format table|json
    Locale:   --adults <N>  --class economy|premium-economy|business|first
              --currency <NAME>  --lang <BCP47>  --country <ISO2>

  graph --from <CODE> --to <CODE> --date <YYYY-MM-DD>
    Options:  --months <N>  (number of months to scan, default 3)
              --adults --class --stops --currency --lang --country --format

  dgrid --from <CODE> --to <CODE>
        --dep-start <DATE> --dep-end <DATE>
        --ret-start <DATE> --ret-end <DATE>
    Options:  --adults --class --stops --currency --lang --country --format

  offer --from <CODE> --to <CODE> --date <YYYY-MM-DD> [--return <DATE>]
    (same filters as search)

  select --from <CODE> --to <CODE> --date <YYYY-MM-DD> [--return <DATE>]
    Interactively pick outbound (and return) flights by number, then an
    offer; prints the booking URL. Enter a number, or 'q' to cancel.

  cheap --from <CODE> --to <CODE> --date <YYYY-MM-DD>
    Options:  --months <N>       (months to scan, default 3)
              --trip-days <N>    (round-trip length in nights; omit for one-way)
              --adults --class --currency --lang --country

  mcity --leg FROM,TO,DATE [--leg FROM,TO,DATE ...]
    Options:  --sort best|price|duration
              --adults --class --currency --lang --country --format

  explore --from <CODE> [--to <CODE|REGION>]
    Options:  --month <1-12>       (travel month; omit for any)
              --duration week|weekend|2weeks
              --budget <N>         (max price in chosen currency)
              --interest <NAME|/m/MID>
                known names: outdoors, beaches, museums, history, skiing, climbing
                aliases: nature, beach, art, heritage, snow, rock-climbing
                raw MID: /m/01rwk  (any Knowledge-Graph MID accepted)
              --max-flight-hours <N>
              --carry-on <N>  --checked <N>
              --adults --class --currency --lang --country --format

  deals --from <CODE> --out <YYYY-MM-DD> --ret <YYYY-MM-DD>
    Options:  --nonstop  --max-hours <N>
              --adults --class --currency --lang --country --format

  quit / exit

Tip: type '<command> --help' for full clap-generated details on any command."
    );
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
                    print_repl_help();
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
    fn repl_parse_help_flag_returns_display_help() {
        // `--help` on any subcommand produces DisplayHelp.  The REPL catches this
        // via `e.print()` and continues without calling process::exit.
        let result = parse(&["search", "--help"]);
        assert!(
            result.is_err(),
            "--help must produce an error (DisplayHelp)"
        );
        let e = result.unwrap_err();
        assert_eq!(
            e.kind(),
            ErrorKind::DisplayHelp,
            "expected DisplayHelp, got: {:?}",
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
    fn repl_parse_search_detail_and_co2_flags() {
        let rc = parse(&[
            "search",
            "--from",
            "LUX",
            "--to",
            "BCN",
            "--date",
            "2026-09-01",
            "--show-co2",
            "--detail",
        ])
        .expect("search with --show-co2 --detail should parse");
        match rc.command {
            Commands::Search(args) => {
                assert!(args.show_co2);
                assert!(args.detail);
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
