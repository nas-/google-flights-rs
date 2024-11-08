use anyhow::Result;
use chrono::NaiveDate;
use gflights::parsers::{
    common::{
        FixedFlights, FlightTimes, Location, StopOptions, StopoverDuration, TotalDuration,
        TravelClass, Travelers,
    },
    flight_response::ItineraryContainer,
};

use gflights::requests::{api::ApiClient, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let departure = Location::new("MAD", 1, Some("Madrid".to_string()));
    let destination = Location::new("MEX", 1, Some("Mexico city".to_string()));
    let departing_date = NaiveDate::parse_from_str("2025-08-10", "%Y-%m-%d").unwrap();
    let return_date: Option<NaiveDate> =
        Some(NaiveDate::parse_from_str("2025-08-30", "%Y-%m-%d").unwrap());
    /*
    Pass None instead for one-way flight.
    let return_date: Option<NaiveDate> = None;
    */
    let departing_times = FlightTimes::default(); // No filters to flights times.
    let return_times = FlightTimes::default(); // No filters to flights times.
    let stopover_max = StopoverDuration::UNLIMITED;
    let duration_max = TotalDuration::UNLIMITED;

    //We want results in euro, so we can avoid providing a currenct, as euros are the default.
    let currency = None;

    let config = Config::new(
        departing_date,
        departure,
        destination,
        StopOptions::All,
        TravelClass::Economy,
        return_date,
        Travelers::new([1, 0, 0, 0].to_vec()),
        departing_times,
        return_times,
        stopover_max,
        duration_max,
        currency.clone(),
    );

    // // Or, shorter...
    // let config = Config{
    //     departing_date,
    //     departure,
    //     destination,
    //     return_date,
    //     ..Default::default()
    // };
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
        first_flight.itinerary_cost.trip_cost,
        currency.clone().unwrap_or_default()
    );
    fixed_flights.add_element(first_flight)?;

    //If one-way flight, just quit
    match return_date {
        Some(_) => {}
        None => {
            println!("Flight url {:?}", config.to_flight_url());
            println!("No return date specified, so one_way flight. Quitting.");
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
        second_flight.itinerary_cost.trip_cost,
        currency.clone().unwrap_or_default()
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
