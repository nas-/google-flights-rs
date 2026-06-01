use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::parsers::flight_response::{FlightInfo, ItineraryContainer};

/// Fixed flights is a vector of ItineraryContainer.
/// It has a maximum number of elements, defined by the type of flight that needs to be searched.
///
/// Uses a standard [`std::sync::Mutex`] because all call sites hold the guard only briefly
/// and no async code yields while the lock is held.
#[derive(Clone, Debug)]
pub struct FixedFlights {
    flights: Arc<Mutex<Vec<ItineraryContainer>>>,
    max_elements: usize,
}

impl Default for FixedFlights {
    fn default() -> Self {
        Self::new(2_usize)
    }
}

impl FixedFlights {
    pub fn new(max_elements: usize) -> Self {
        FixedFlights {
            flights: Arc::new(Mutex::new(Vec::new())),
            max_elements,
        }
    }

    pub fn add_element(&self, element: ItineraryContainer) -> Result<()> {
        if self.is_full() {
            return Err(anyhow!("Vector max number of elements reached"));
        }

        let mut flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(anyhow!("Failed to acquire lock")),
        };
        flights.push(element);
        Ok(())
    }

    pub fn maybe_get_nth_flight_info(&self, nth: usize) -> Option<Vec<FlightInfo>> {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; returning None");
                return None;
            }
        };
        flights.get(nth).map(|f| f.itinerary.flight_details.clone())
    }

    pub fn is_full(&self) -> bool {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; assuming not full");
                return false;
            }
        };
        flights.len() >= self.max_elements
    }

    pub fn get_departure_token(&self) -> Option<String> {
        let flights = match self.flights.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("FixedFlights mutex was poisoned; returning None");
                return None;
            }
        };
        let length = flights.len().checked_sub(1)?;
        flights.get(length).map(|f| f.get_departure_token())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::parsers::flight_response::{Itinerary, ItineraryCost};

    use super::*;

    #[test]
    fn fixed_flights_add_element_enforces_max() {
        let ff = FixedFlights::new(1);
        let dummy = || ItineraryContainer {
            itinerary: Itinerary {
                flight_by: "XX".to_owned(),
                flight_details: vec![],
                total_time_minutes: 0,
                connection_info: None,
                emissions: None,
            },
            itinerary_cost: ItineraryCost {
                departure_token: "tok".to_owned(),
                trip_cost: None,
            },
            departure_protobuf: String::new(),
        };
        assert!(ff.add_element(dummy()).is_ok());
        assert!(ff.is_full());
        assert!(ff.add_element(dummy()).is_err());
    }

    #[test]
    fn fixed_flights_get_departure_token_returns_last() {
        let ff = FixedFlights::new(2);
        let make = |tok: &str| ItineraryContainer {
            itinerary: Itinerary {
                flight_by: "XX".to_owned(),
                flight_details: vec![],
                total_time_minutes: 0,
                connection_info: None,
                emissions: None,
            },
            itinerary_cost: ItineraryCost {
                departure_token: tok.to_owned(),
                trip_cost: None,
            },
            departure_protobuf: String::new(),
        };
        assert_eq!(ff.get_departure_token(), None);
        ff.add_element(make("first_token")).unwrap();
        assert_eq!(ff.get_departure_token(), Some("first_token".to_owned()));
        ff.add_element(make("second_token")).unwrap();
        assert_eq!(ff.get_departure_token(), Some("second_token".to_owned()));
    }

    #[test]
    fn fixed_flights_maybe_get_nth_flight_info() {
        let ff = FixedFlights::new(2);
        assert!(ff.maybe_get_nth_flight_info(0).is_none());
        let dummy = ItineraryContainer {
            itinerary: Itinerary {
                flight_by: "BA".to_owned(),
                flight_details: vec![],
                total_time_minutes: 120,
                connection_info: None,
                emissions: None,
            },
            itinerary_cost: ItineraryCost {
                departure_token: "tok".to_owned(),
                trip_cost: None,
            },
            departure_protobuf: String::new(),
        };
        ff.add_element(dummy).unwrap();
        let info = ff.maybe_get_nth_flight_info(0);
        assert!(info.is_some());
        assert_eq!(info.unwrap().len(), 0);
    }
}
