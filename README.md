# google-flights-rs
Unofficial API for google flights, impemented in Rust



# Use
## Request for a single itinerary & offers

```rust
//main.rs
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

#[tokyo::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let departure = Location::new("MAD", 1, Some("Madrid".to_string()));
    let destination = Location::new("MEX", 1, Some("Mexico city".to_string()));
    let departing_date = NaiveDate::parse_from_str("2024-08-10", "%Y-%m-%d").unwrap();
    let return_date: Option<NaiveDate> =
        Some(NaiveDate::parse_from_str("2024-08-30", "%Y-%m-%d").unwrap()); // No Pass None for one-way flight.
    let departing_times = FlightTimes::default(); // No filters to flights times.
    let return_times = FlightTimes::default(); // No filters to flights times.
    let stopover_max = StopoverDuration::UNLIMITED;
    let duration_max = TotalDuration::UNLIMITED;

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
    );
    let fixed_flights = FixedFlights::new(2_usize);
    let response = client.request_flights(&config, &fixed_flights).await?;
    let first_flight: ItineraryContainer = response.responses
        .into_iter()
        .flat_map(|response| response.maybe_get_all_flights())
        .flatten()
        .next()
        .unwrap();
    println!("Itinerary {:?}", first_flight.itinerary);
    println!("Price {:?}", first_flight.itinerary_cost.trip_cost);

    //If one-way flight, just quit
    match return_date {
        Some(_) => {}
        None => return Ok(()),
    }
    
    fixed_flights.add_element(first_flight)?;
    let second_flight_response = client.request_flights(&config, &fixed_flights).await?;
    let second_flight: ItineraryContainer = second_flight_response.responses
        .into_iter()
        .flat_map(|response| response.maybe_get_all_flights())
        .flatten()
        .next()
        .unwrap();

    println!("Return flight itinerary {:?}", second_flight.itinerary);
    println!("Price {:?}", second_flight.itinerary_cost.trip_cost);
    println!("Itinerary link {:?}", config.to_flight_url());

    fixed_flights.add_element(second_flight)?;
    // ask for offers:
    let offers_vec = client.request_offer(&config, &fixed_flights).await?;
    let offers = offers_vec.response.first().unwrap().get_offer_prices().unwrap();
    println!("Offers for this flight: {:?}", offers);

    Ok(())
}
```


## Request Flight Graph

```rust
use anyhow::Result;
use chrono::{Months, NaiveDate};
use parsers::{
    common::{
        FlightTimes, Location, StopOptions, StopoverDuration, TotalDuration,
        TravelClass, Travelers,
    }, flight_response::CheaperTravelDifferentDates,
};

use crate::requests::{api::ApiClient, config::Config};

#[tokyo::main]
async fn main() -> Result<()> {
    let client = ApiClient::new().await;
    let departure = Location::new("MAD", 1, Some("Madrid".to_string()));
    let destination = Location::new("MEX", 1, Some("Mexico city".to_string()));
    let departing_date = NaiveDate::parse_from_str("2024-08-10", "%Y-%m-%d").unwrap();
    let return_date: Option<NaiveDate> =
        Some(NaiveDate::parse_from_str("2024-08-30", "%Y-%m-%d").unwrap()); // No Pass None for one-way flight.
    let departing_times = FlightTimes::default(); // No filters to flights times.
    let return_times = FlightTimes::default(); // No filters to flights times.
    let stopover_max = StopoverDuration::UNLIMITED;
    let duration_max = TotalDuration::UNLIMITED;

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
    );
    let months = Months::new(5);
    let response = client.request_graph(&config,months).await?;

    let graph: Vec<CheaperTravelDifferentDates> = response.get_all_graphs();
    // {proposed_departure_date:Date, proposed_trip_cost:Option<{"trip_cost":cost}>}
    println!("Graph: {:?}",graph);
    Ok(())
}
```



