use anyhow::Result;
use chrono::NaiveDate;
use gflights::{
    parsers::{
        common::{FixedFlights, Location, Travelers},
        flight_response::ItineraryContainer,
    },
    requests::config::TripType,
};

use gflights::requests::{api::ApiClient, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let departure = Location::new("MAD", 1, Some("Madrid".to_string()));
    let destination = Location::new("MEX", 1, Some("Mexico city".to_string()));
    let departing_date = NaiveDate::parse_from_str("2025-08-10", "%Y-%m-%d").unwrap();
    let return_date = NaiveDate::parse_from_str("2025-08-30", "%Y-%m-%d").unwrap();

    let config = Config::builder()
        .departing_date(departing_date)
        .departure(departure)
        .destination(destination)
        .return_date(return_date)
        .travelers(Travelers::new([1, 0, 0, 0].to_vec()))
        .build()?;

    let fixed_flights = FixedFlights::new(2_usize);

    let response = client.request_flights(&config, &fixed_flights).await?;
    let maybe_next_flight: Option<ItineraryContainer> = response
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
    fixed_flights.add_element(first_flight)?;

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

    let second_flight_response = client.request_flights(&config, &fixed_flights).await?;
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

    fixed_flights.add_element(second_flight)?;
    // ask for offers:
    let offers_vec = client.request_offer(&config, &fixed_flights).await?;
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
