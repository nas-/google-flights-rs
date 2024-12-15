use anyhow::Result;
use chrono::{Months, NaiveDate};
use gflights::parsers::{
    common::{Location, Travelers},
    flight_response::CheaperTravelDifferentDates,
};

use gflights::requests::{
    api::ApiClient,
    config::{Config, Currency},
};

#[tokio::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let departure = Location::new("MAD", 1, Some("Madrid".to_string()));
    let destination = Location::new("MEX", 1, Some("Mexico city".to_string()));
    let departing_date = NaiveDate::parse_from_str("2024-12-10", "%Y-%m-%d").unwrap();
    let return_date = NaiveDate::parse_from_str("2024-12-30", "%Y-%m-%d").unwrap();

    //Set currency to USDollar, default is euros.
    let config = Config::builder()
        .departing_date(departing_date)
        .departure(departure)
        .destination(destination)
        .return_date(return_date)
        .currency(Currency::USDollar)
        .travelers(Travelers::new([1, 0, 0, 0].to_vec()))
        .build()?;

    let months = Months::new(5);
    let response = client.request_graph(&config, months).await?;

    let graph: Vec<CheaperTravelDifferentDates> = response.get_all_graphs();
    // {proposed_departure_date:Date, proposed_trip_cost:Option<{"trip_cost":cost}>}
    println!("Graph: {:?}", graph);

    let lowest_cost = graph
        .iter()
        .filter(|graph| graph.proposed_trip_cost.is_some())
        .min_by(|a, b| {
            a.proposed_trip_cost
                .as_ref()
                .unwrap()
                .trip_cost
                .price
                .partial_cmp(&b.proposed_trip_cost.as_ref().unwrap().trip_cost.price)
                .unwrap()
        });

    println!(
        "Lowest cost itinerarty:{} {:?}",
        lowest_cost.expect("No prices found for this itinerary"),
        config.currency
    );

    Ok(())
}
