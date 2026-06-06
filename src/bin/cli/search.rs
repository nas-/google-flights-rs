use anyhow::Result;
use clap::Parser;
use gflights::parsers::common::{AirlineFilter, SortOrder, StopoverDuration};
use gflights::requests::api::ApiClient;

use super::{build_config, CommonArgs, OutputFormat};
use gflights::requests::config::Config;

/// Arguments for the `search` subcommand.
#[derive(Parser, Debug)]
pub struct SearchArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Sort order.
    #[arg(long, default_value = "best")]
    pub sort: SortOrder,

    /// Minimum layover duration in minutes (rounded up to the next 30 min interval).
    #[arg(long)]
    pub min_layover: Option<u32>,

    /// Maximum layover duration in minutes (rounded up to the next 30 min interval).
    #[arg(long)]
    pub max_layover: Option<u32>,

    /// Restrict results to lower-CO₂ emissions flights.
    #[arg(long)]
    pub lower_emissions: bool,

    /// Airline IATA code (e.g. LX, LH) or alliance name (ONEWORLD, SKYTEAM, STAR_ALLIANCE)
    /// to include. May be repeated for multiple airlines/alliances.
    #[arg(long = "airline")]
    pub airlines: Vec<AirlineFilter>,

    /// Airline IATA code or alliance name to exclude.
    /// May be repeated for multiple airlines/alliances.
    #[arg(long = "exclude-airline")]
    pub exclude_airlines: Vec<AirlineFilter>,

    /// Require a connection through this IATA airport code (e.g. CDG, AMS).
    /// May be repeated for multiple airports.
    #[arg(long = "via")]
    pub connecting_airports: Vec<String>,

    /// Show a CO₂ emissions column (kg per passenger).
    #[arg(long = "show-co2")]
    pub show_co2: bool,

    /// Show detailed info: layover airports and +1 marker for next-day arrivals.
    #[arg(long)]
    pub detail: bool,
}

pub async fn cmd_search(args: SearchArgs, client: &ApiClient) -> Result<()> {
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
    // Strict "via": Google's other_flights container leaks non-stops that skip
    // the requested connecting airport, so filter client-side.
    let mut flights = results.get_all_flights_via(&config.connecting_airports);

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
            // Build header dynamically depending on flags.
            if args.show_co2 {
                println!(
                    "{:<8}  {:>6}  {:>5}  {:>5}  {:>7}  ROUTE",
                    "AIRLINE", "PRICE", "STOPS", "MINS", "CO2(kg)"
                );
            } else {
                println!(
                    "{:<8}  {:>6}  {:>5}  {:>5}  ROUTE",
                    "AIRLINE", "PRICE", "STOPS", "MINS"
                );
            }
            println!("{}", "-".repeat(if args.show_co2 { 70 } else { 60 }));

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

                // "+1" marker when the final leg arrives the calendar day after departure.
                let next_day = if args.detail && f.itinerary.arrives_next_day() {
                    " +1"
                } else {
                    ""
                };

                let route = format!("{}→{}{}", from, to, next_day);

                if args.show_co2 {
                    let co2_str = f
                        .itinerary
                        .emissions
                        .as_ref()
                        .and_then(|e| e.co2_this_flight_g)
                        .map(|g| format!("{}", g / 1000))
                        .unwrap_or_else(|| "—".into());
                    println!(
                        "{:<8}  {:>6}  {:>5}  {:>5}  {:>7}  {}",
                        f.itinerary.flight_by,
                        price,
                        f.itinerary.stop_count(),
                        f.itinerary.total_time_minutes,
                        co2_str,
                        route,
                    );
                } else {
                    println!(
                        "{:<8}  {:>6}  {:>5}  {:>5}  {}",
                        f.itinerary.flight_by,
                        price,
                        f.itinerary.stop_count(),
                        f.itinerary.total_time_minutes,
                        route,
                    );
                }

                // Detail row: layover airports for multi-stop itineraries.
                if args.detail {
                    if let Some(conns) = &f.itinerary.connection_info {
                        if !conns.is_empty() {
                            let via_parts: Vec<String> = conns
                                .iter()
                                .map(|c| {
                                    format!(
                                        "{} ({} min)",
                                        c.arrival_airport, c.connection_time_minutes
                                    )
                                })
                                .collect();
                            println!("             via {}", via_parts.join(" → "));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Extension trait used only by search to apply sort order after build.
trait WithSortOrder {
    fn with_sort_order(self, sort: SortOrder) -> Self;
}

impl WithSortOrder for Config {
    fn with_sort_order(mut self, sort: SortOrder) -> Self {
        self.sort_order = sort;
        self
    }
}
