use anyhow::Result;
use clap::Parser;
use gflights::requests::api::ApiClient;

use super::{build_config, CommonArgs, OutputFormat};

/// Arguments for the `offer` subcommand.
#[derive(Parser, Debug)]
pub struct OfferArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}

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

pub async fn cmd_offer(args: OfferArgs, client: &ApiClient) -> Result<()> {
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
