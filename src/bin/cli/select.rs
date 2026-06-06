//! Interactive `select` flow: pick an outbound flight, then (for round trips)
//! a return flight, then a booking offer — and print the resolved booking URL.
//!
//! This drives the same `fixed_flights` two-step path the `offer` command uses,
//! but lets the user choose each leg by number. It works both as a one-shot
//! subcommand and inside the REPL; numeric choices are read from stdin.

use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};

use gflights::parsers::flight_response::ItineraryContainer;
use gflights::requests::api::ApiClient;

use super::{build_config, CommonArgs};

/// Arguments for the `select` subcommand.
#[derive(Parser, Debug)]
pub struct SelectArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}

/// A parsed user selection at an interactive prompt.
#[derive(Debug, PartialEq, Eq)]
pub enum Selection {
    /// A valid, zero-based index into the option list.
    Index(usize),
    /// The user cancelled (blank line, `q`, or `quit`).
    Cancel,
    /// Unparseable or out-of-range input.
    Invalid,
}

/// Parse a line of user input into a [`Selection`] given the number of options.
///
/// Accepts a 1-based number in `1..=max` (returned as a 0-based index), treats
/// blank input or `q`/`quit` as cancellation, and anything else as invalid.
pub fn parse_selection(input: &str, max: usize) -> Selection {
    let t = input.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("q") || t.eq_ignore_ascii_case("quit") {
        return Selection::Cancel;
    }
    match t.parse::<usize>() {
        Ok(n) if n >= 1 && n <= max => Selection::Index(n - 1),
        _ => Selection::Invalid,
    }
}

/// Prompt on stdout and read a selection from stdin, re-prompting on invalid
/// input. Returns `Ok(None)` if the user cancels or stdin reaches EOF.
fn read_selection(prompt: &str, max: usize) -> Result<Option<usize>> {
    loop {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            return Ok(None); // EOF
        }
        match parse_selection(&line, max) {
            Selection::Index(i) => return Ok(Some(i)),
            Selection::Cancel => return Ok(None),
            Selection::Invalid => {
                println!("Enter a number between 1 and {max}, or 'q' to cancel.");
            }
        }
    }
}

/// Print a numbered table of flight options.
fn print_flight_options(flights: &[ItineraryContainer]) {
    println!(
        "{:>3}  {:<8}  {:>6}  {:>5}  {:>5}  ROUTE",
        "#", "AIRLINE", "PRICE", "STOPS", "MINS"
    );
    println!("{}", "-".repeat(50));
    for (i, f) in flights.iter().enumerate() {
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
            "{:>3}  {:<8}  {:>6}  {:>5}  {:>5}  {}→{}",
            i + 1,
            f.itinerary.flight_by,
            price,
            f.itinerary.stop_count(),
            f.itinerary.total_time_minutes,
            from,
            to,
        );
    }
}

/// Pick one flight from a fresh search, displaying numbered options.
/// Returns `Ok(None)` if there are no flights or the user cancels.
async fn pick_leg(
    label: &str,
    config: &gflights::requests::config::Config,
    client: &ApiClient,
) -> Result<Option<ItineraryContainer>> {
    let result = client.request_flights(config).await?;
    let flights = result.get_all_flights();
    if flights.is_empty() {
        eprintln!("No {label} flights found.");
        return Ok(None);
    }
    println!("\n{} flights:", capitalize(label));
    print_flight_options(&flights);
    let prompt = format!("Select {label} flight [1-{}, q to cancel]: ", flights.len());
    match read_selection(&prompt, flights.len())? {
        Some(idx) => Ok(flights.into_iter().nth(idx)),
        None => Ok(None),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub async fn cmd_select(args: SelectArgs, client: &ApiClient) -> Result<()> {
    let config = build_config(&args.common, client).await?;

    // Step 1 — outbound leg.
    let outbound = match pick_leg("outbound", &config, client).await? {
        Some(f) => f,
        None => {
            println!("Cancelled.");
            return Ok(());
        }
    };
    config.fixed_flights.add_element(outbound)?;

    // Step 2 — return leg (round trips only). With the outbound fixed, a fresh
    // search returns the matching return candidates.
    if config.return_date.is_some() {
        let ret = match pick_leg("return", &config, client).await? {
            Some(f) => f,
            None => {
                println!("Cancelled.");
                return Ok(());
            }
        };
        config.fixed_flights.add_element(ret)?;
    }

    // Step 3 — booking offers for the selected itinerary.
    let offers = client.request_offer(&config).await?;
    let mut groups: Vec<_> = offers
        .response
        .iter()
        .flat_map(|r| &r.offers)
        .filter(|o| o.price.is_some())
        .collect();
    groups.sort_by_key(|o| o.price.unwrap_or(i32::MAX));

    if groups.is_empty() {
        eprintln!("No booking offers found for this itinerary.");
        return Ok(());
    }

    println!("\nBooking offers:");
    println!("{:>3}  {:<30}  {:>8}", "#", "AIRLINE(S)", "PRICE");
    println!("{}", "-".repeat(45));
    for (i, o) in groups.iter().enumerate() {
        println!(
            "{:>3}  {:<30}  {:>8}",
            i + 1,
            o.airline_names.join(", "),
            o.price.unwrap_or(0),
        );
    }
    let offer = match read_selection(
        &format!("Select offer to book [1-{}, q to cancel]: ", groups.len()),
        groups.len(),
    )? {
        Some(idx) => groups[idx],
        None => {
            println!("Cancelled.");
            return Ok(());
        }
    };

    // Step 4 — resolve the booking URL from the chosen offer (or a sub-option).
    let token = offer.click_token.as_deref().or_else(|| {
        offer
            .sub_options
            .iter()
            .find_map(|s| s.click_token.as_deref())
    });
    match token {
        Some(t) => {
            let url = client.resolve_booking_url(t).await?;
            println!("\nBooking URL:\n{url}");
        }
        None => println!("No booking link available for this offer."),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_selection_valid_indices_are_zero_based() {
        assert_eq!(parse_selection("1", 5), Selection::Index(0));
        assert_eq!(parse_selection("5", 5), Selection::Index(4));
        assert_eq!(parse_selection("  3 ", 5), Selection::Index(2));
    }

    #[test]
    fn parse_selection_cancel_inputs() {
        assert_eq!(parse_selection("", 5), Selection::Cancel);
        assert_eq!(parse_selection("   ", 5), Selection::Cancel);
        assert_eq!(parse_selection("q", 5), Selection::Cancel);
        assert_eq!(parse_selection("Q", 5), Selection::Cancel);
        assert_eq!(parse_selection("quit", 5), Selection::Cancel);
    }

    #[test]
    fn parse_selection_out_of_range_is_invalid() {
        assert_eq!(parse_selection("0", 5), Selection::Invalid);
        assert_eq!(parse_selection("6", 5), Selection::Invalid);
        assert_eq!(parse_selection("99", 5), Selection::Invalid);
    }

    #[test]
    fn parse_selection_non_numeric_is_invalid() {
        assert_eq!(parse_selection("abc", 5), Selection::Invalid);
        assert_eq!(parse_selection("1x", 5), Selection::Invalid);
        assert_eq!(parse_selection("-1", 5), Selection::Invalid);
    }

    #[test]
    fn capitalize_works() {
        assert_eq!(capitalize("outbound"), "Outbound");
        assert_eq!(capitalize("return"), "Return");
        assert_eq!(capitalize(""), "");
    }
}
