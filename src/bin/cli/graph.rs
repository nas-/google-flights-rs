use anyhow::Result;
use clap::Parser;
use gflights::requests::api::ApiClient;

use super::{build_config, CommonArgs, OutputFormat};

/// Arguments for the `graph` subcommand.
#[derive(Parser, Debug)]
pub struct GraphArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Number of months to show in the price graph.
    #[arg(long, default_value = "3")]
    pub months: u32,
}

pub async fn cmd_graph(args: GraphArgs, client: &ApiClient) -> Result<()> {
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
