use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use gflights::requests::config::TripType;

use gflights::requests::{api::ApiClient, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let today = Utc::now().date_naive();
    let departing_date = today + Duration::days(10);
    let return_date = today + Duration::days(20);

    let config = Config::builder()
        .departure("MAD", &client)
        .await
        .with_context(|| "Failed to set departure airport")?
        .destination("MEX", &client)
        .await
        .with_context(|| "Failed to set destination airport")?
        .departing_date(departing_date)
        .return_date(return_date)
        .build()
        .with_context(|| "Failed to build the configuration")?;

    let flight_response = client
        .request_flights(&config)
        .await
        .with_context(|| "Failed to request flights")?;

    let first_flight = flight_response
        .responses
        .iter()
        .filter_map(|response| response.maybe_get_all_flights())
        .flatten()
        .next()
        .with_context(|| "No flights found for this request")?;

    println!("Itinerary {:?}", first_flight.itinerary);
    println!(
        "Price {:?} {:?}",
        first_flight.itinerary_cost.trip_cost, config.currency
    );
    config.fixed_flights.add_element(first_flight)?;

    // Check the trip type.

    if config.trip_type == TripType::OneWay {
        println!("Itinerary URL: {}", config.to_flight_url());

        println!("One-way flight detected. Exiting.");
        return Ok(());
    } else if config.trip_type != TripType::Return {
        println!("Unsupported trip type.");
        return Ok(());
    }

    // Request return flights.

    let return_flight_response = client
        .request_flights(&config)
        .await
        .with_context(|| "Failed to request return flights")?;

    // Find the second (return) flight.

    let maybe_second_flight = return_flight_response
        .responses
        .iter()
        .filter_map(|resp| resp.maybe_get_all_flights())
        .flatten()
        .next();

    let second_flight = match maybe_second_flight {
        Some(flight) => flight,

        None => {
            println!("No return flights found.");

            return Ok(());
        }
    };

    println!("Return Itinerary: {:?}", second_flight.itinerary);

    println!(
        "Price: {:?} {:?}",
        second_flight.itinerary_cost.trip_cost, config.currency
    );

    // Add the second flight to the fixed flights in the config.

    config
        .fixed_flights
        .add_element(second_flight.clone())
        .with_context(|| "Failed to add second flight to fixed flights")?;

    println!("Itinerary URL: {}", config.to_flight_url());

    // ask for offers:
    let offers_response = client
        .request_offer(&config)
        .await
        .with_context(|| "Failed to request offers")?;

    let mut offers: Vec<(Vec<String>, i32)> = offers_response
        .response
        .iter()
        .filter_map(|resp| resp.get_offer_prices())
        .flatten()
        .collect();

    offers.sort_by(|a, b| a.1.cmp(&b.1));

    if offers.is_empty() {
        println!("No offers found");
        return Ok(());
    }
    for (offer, price) in offers {
        println!("Offer: {:?}, Price: {} {:?}", offer, price, config.currency);
    }

    Ok(())
}
