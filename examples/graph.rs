use anyhow::Result;
use chrono::{Months, NaiveDate};
use gflights::parsers::{
    common::{
        FlightTimes, Location, StopOptions, StopoverDuration, TotalDuration, TravelClass, Travelers,
    },
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
    let return_date: Option<NaiveDate> =
        Some(NaiveDate::parse_from_str("2024-12-30", "%Y-%m-%d").unwrap());
    /*
    Pass None instead for one-way flight.
    let return_date: Option<NaiveDate> = None;
    */
    let departing_times = FlightTimes::default(); // No filters to flights times.
    let return_times = FlightTimes::default(); // No filters to flights times.
    let stopover_max = StopoverDuration::UNLIMITED;
    let duration_max = TotalDuration::UNLIMITED;

    //Set currency to USDollar, default is euros.
    let currency = Some(Currency::USDollar);
    let config = Config::new(
        departing_date,
        departure,
        destination,
        StopOptions::OneOrLess,
        TravelClass::Economy,
        return_date,
        Travelers::new([1, 0, 0, 0].to_vec()),
        departing_times,
        return_times,
        stopover_max,
        duration_max,
        currency.clone(),
    );
    // Or, shorter...
    // let config = Config{
    //     departing_date,
    //     departure,
    //     destination,
    //     stop_options:StopOptions::OneOrLess,
    //     return_date,
    //     ..Default::default()
    // };
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
        currency.unwrap_or_default()
    );

    Ok(())
}
