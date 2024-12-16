use anyhow::Result;
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
        .await?
        .destination("MEX", &client)
        .await?
        .departing_date(departing_date)
        .return_date(return_date)
        .build()?;

    let response = client.request_flights(&config).await?;
    let maybe_next_flight = response
        .responses
        .into_iter()
        .flat_map(|response| response.maybe_get_all_flights())
        .flatten()
        .next();

    let first_flight = match maybe_next_flight {
        Some(flight) => flight,
        None => {
            print!("No flights found for this request");
            return Ok(());
        }
    };
    println!("Itinerary {:?}", first_flight.itinerary);
    println!(
        "Price {:?} {:?}",
        first_flight.itinerary_cost.trip_cost, config.currency
    );
    config.fixed_flights.add_element(first_flight)?;

    //If one-way flight, just quit
    match config.trip_type {
        TripType::Return => {}
        TripType::OneWay => {
            println!("Flight url {:?}", config.to_flight_url());
            println!("No return date specified, so one_way flight. Quitting.");
            return Ok(());
        }
        _ => {
            println!("Unsupported trip type");
            return Ok(());
        }
    }

    let second_flight_response = client.request_flights(&config).await?;
    let maybe_second_flight = second_flight_response
        .responses
        .into_iter()
        .flat_map(|response| response.maybe_get_all_flights())
        .flatten()
        .next();

    let second_flight = match maybe_second_flight {
        Some(flight) => flight,
        None => return Ok(()),
    };

    println!("Return flight itinerary {:?}", second_flight.itinerary);
    println!(
        "Price {:?} {:?}",
        second_flight.itinerary_cost.trip_cost, config.currency
    );
    println!("Itinerary link {:?}", config.to_flight_url());

    config.fixed_flights.add_element(second_flight)?;
    // ask for offers:
    let offers_vec = client.request_offer(&config).await?;
    let maybe_offers = offers_vec
        .response
        .first()
        .and_then(|response| response.get_offer_prices());
    if let Some(offers) = maybe_offers {
        println!("Offers for this flight: {:?}", offers);
    } else {
        println!("No offers for this flight");
    }

    Ok(())
}
