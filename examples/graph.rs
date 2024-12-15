use anyhow::Result;
use chrono::{Duration, Months, Utc};

use gflights::requests::{
    api::ApiClient,
    config::{Config, Currency},
};

#[tokio::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;

    let today = Utc::now().date_naive();
    let departing_date = today + Duration::days(10);
    let return_date = today + Duration::days(20);

    //Set currency to USDollar, default is euros.
    let config: Config = Config::builder()
        .departure("MAD", &client)
        .await?
        .destination("MEX", &client)
        .await?
        .departing_date(departing_date)
        .return_date(return_date)
        .currency(Currency::USDollar)
        .build()?;

    let months = Months::new(5);
    let response = client.request_graph(&config, months).await?;

    let graph = response.get_all_graphs();
    // {proposed_departure_date:Date, proposed_trip_cost:Option<{"trip_cost":cost}>}
    println!("Graph: {:?}", graph);

    let lowest_cost = graph
        .iter()
        .flat_map(|x| x.clone().maybe_get_date_price())
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    if let Some(date_price) = lowest_cost {
        println!(
            "Lowest cost itinerary: Departure {:?} {} {:?}",
            date_price.0, date_price.1, config.currency
        );
    } else {
        println!("No prices found for this itinerary");
    }
    Ok(())
}
