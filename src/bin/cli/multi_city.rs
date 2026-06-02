use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use gflights::parsers::common::{SortOrder, TravelClass, Travelers};
use gflights::requests::api::ApiClient;
use gflights::requests::config::{Currency, MultiCityConfig};

use super::OutputFormat;

/// A single leg specified on the command line.
///
/// Use the repeatable `--leg FROM TO DATE` flag to add each leg.
#[derive(Debug, Clone)]
pub struct LegArg {
    pub from: String,
    pub to: String,
    pub date: NaiveDate,
}

impl std::str::FromStr for LegArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.splitn(3, ',').collect();
        if parts.len() != 3 {
            anyhow::bail!("leg must be FROM,TO,DATE (e.g. LUX,FCO,2026-09-10), got: {s}");
        }
        Ok(LegArg {
            from: normalize_location(parts[0].trim()),
            to: normalize_location(parts[1].trim()),
            date: NaiveDate::parse_from_str(parts[2].trim(), "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("invalid date '{}': {e}", parts[2]))?,
        })
    }
}

/// Normalize a location string for use with [`MultiCityConfigBuilder::add_leg`].
///
/// 3-char all-alphabetic tokens are treated as IATA airport codes and uppercased
/// (e.g. `"lux"` → `"LUX"`).  Anything else is a city or region name and is
/// preserved as-is so the city-lookup API can match it correctly
/// (e.g. `"London"` stays `"London"`, not `"LONDON"`).
fn normalize_location(s: &str) -> String {
    if s.len() == 3 && s.chars().all(|c| c.is_ascii_alphabetic()) {
        s.to_uppercase()
    } else {
        s.to_string()
    }
}

/// Arguments for the `mcity` subcommand.
#[derive(Parser, Debug)]
pub struct MultiCityArgs {
    /// Leg in FROM,TO,DATE format (e.g. LUX,FCO,2026-09-10).
    /// Repeat for each leg (minimum 2).
    #[arg(long = "leg", required = true, num_args = 1)]
    pub legs: Vec<LegArg>,

    /// Number of adult passengers.
    #[arg(long, default_value = "1")]
    pub adults: u32,

    /// Travel class.
    #[arg(long, default_value = "economy")]
    pub class: TravelClass,

    /// Sort order.
    #[arg(long, default_value = "best")]
    pub sort: SortOrder,

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

pub async fn cmd_multi_city(args: MultiCityArgs, client: &ApiClient) -> Result<()> {
    if args.legs.len() < 2 {
        anyhow::bail!("multi-city requires at least 2 legs");
    }

    let travelers = Travelers::new(vec![args.adults as i32, 0, 0, 0])?;

    let mut builder = MultiCityConfig::builder()
        .travellers(travelers)
        .travel_class(args.class)
        .sort_order(args.sort)
        .currency(args.currency)
        .language(args.lang)
        .country(args.country);

    for leg in &args.legs {
        builder = builder
            .add_leg(&leg.from, &leg.to, leg.date, client)
            .await?;
    }

    let config = builder.build()?;
    let results = client.request_multi_city_flights(&config).await?;
    let flights = results.get_all_flights();

    if flights.is_empty() {
        eprintln!("No flights found.");
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&flights)?);
        }
        OutputFormat::Table => {
            // Print each flight with its leg route for context.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leg_arg_parses_valid_input() {
        let leg: LegArg = "LUX,FCO,2026-09-10".parse().unwrap();
        assert_eq!(leg.from, "LUX");
        assert_eq!(leg.to, "FCO");
        assert_eq!(leg.date.to_string(), "2026-09-10");
    }

    #[test]
    fn leg_arg_uppercases_iata_codes() {
        // lowercase 3-char codes → uppercased IATA
        let leg: LegArg = "lux,fco,2026-09-10".parse().unwrap();
        assert_eq!(leg.from, "LUX");
        assert_eq!(leg.to, "FCO");
    }

    #[test]
    fn leg_arg_preserves_city_names() {
        // city names (>3 chars) must keep original case for the city-lookup API
        let leg: LegArg = "MXP,London,2026-09-12".parse().unwrap();
        assert_eq!(leg.from, "MXP");
        assert_eq!(leg.to, "London");
    }

    #[test]
    fn leg_arg_preserves_mixed_case_city() {
        let leg: LegArg = "lux,New York,2026-12-01".parse().unwrap();
        assert_eq!(leg.from, "LUX");
        assert_eq!(leg.to, "New York");
    }

    #[test]
    fn leg_arg_rejects_missing_parts() {
        assert!("LUX,FCO".parse::<LegArg>().is_err());
    }

    #[test]
    fn leg_arg_rejects_invalid_date() {
        assert!("LUX,FCO,not-a-date".parse::<LegArg>().is_err());
    }
}
