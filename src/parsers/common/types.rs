use anyhow::Result;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::SerializeToWeb;

/// This is the type of place. It can be an airport, a city, a region, etc.
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone, Copy, Default)]
#[repr(i32)]
#[serde(untagged)]
pub enum PlaceType {
    #[default]
    Unspecified = 0,
    Airport = 1,
    MaybeRegion = 3,
    RegionMaybe = 4,
    City = 5,
}

impl From<i32> for PlaceType {
    fn from(value: i32) -> Self {
        match value {
            0 => PlaceType::Unspecified,
            1 => PlaceType::Airport,
            3 => PlaceType::MaybeRegion,
            4 => PlaceType::RegionMaybe,
            5 => PlaceType::City,
            _ => {
                tracing::warn!(
                    value,
                    "Unknown PlaceType discriminant; treating as Unspecified"
                );
                PlaceType::Unspecified
            }
        }
    }
}

/// Travel class. It can be economy, premium economy, business or first class.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum TravelClass {
    #[default]
    Economy = 1,
    PremiumEconomy = 2,
    Business = 3,
    First = 4,
}

impl SerializeToWeb for TravelClass {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!("{}", *self as i32))
    }
}

impl From<i32> for TravelClass {
    fn from(value: i32) -> Self {
        match value {
            1 => TravelClass::Economy,
            2 => TravelClass::PremiumEconomy,
            3 => TravelClass::Business,
            4 => TravelClass::First,
            _ => {
                tracing::warn!(
                    value,
                    "Unknown TravelClass discriminant; defaulting to Economy"
                );
                TravelClass::Economy
            }
        }
    }
}

/// Sort order for flight search results.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum SortOrder {
    /// Google's default: best combination of price, duration, and convenience.
    #[default]
    Best = 1,
    /// Sort by total price, cheapest first.
    Price = 2,
    /// Sort by total journey duration, shortest first.
    Duration = 3,
    /// Sort by departure time, earliest first.
    DepartureTime = 4,
    /// Sort by arrival time, earliest first.
    ArrivalTime = 5,
}

impl SortOrder {
    /// Returns the sort discriminant to send to the Google Flights backend.
    ///
    /// `DepartureTime` (4) and `ArrivalTime` (5) are not recognised as valid
    /// sort modes by the server and cause it to return an empty response.
    /// Fall back to `Best` for those two and handle the ordering client-side.
    pub fn server_sort(self) -> SortOrder {
        match self {
            SortOrder::DepartureTime | SortOrder::ArrivalTime => SortOrder::Best,
            other => other,
        }
    }
}

/// Stop options. It can be all, no stop, one or less, two or less.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, ValueEnum, Default)]
pub enum StopOptions {
    #[default]
    All = 0,
    NoStop = 1,
    OneOrLess = 2,
    TwoOrLess = 3,
}

impl SerializeToWeb for StopOptions {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!("{}", *self as i32))
    }
}

impl From<i32> for StopOptions {
    fn from(value: i32) -> Self {
        match value {
            0 => StopOptions::All,
            1 => StopOptions::NoStop,
            2 => StopOptions::OneOrLess,
            3 => StopOptions::TwoOrLess,
            _ => {
                tracing::warn!(value, "Unknown StopOptions discriminant; defaulting to All");
                StopOptions::All
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::parsers::common::SerializeToWeb;

    use super::*;

    #[test]
    fn sort_order_discriminant_values() {
        assert_eq!(SortOrder::Best as i32, 1);
        assert_eq!(SortOrder::Price as i32, 2);
        assert_eq!(SortOrder::Duration as i32, 3);
        assert_eq!(SortOrder::DepartureTime as i32, 4);
        assert_eq!(SortOrder::ArrivalTime as i32, 5);
    }

    #[test]
    fn sort_order_default_is_best() {
        assert!(matches!(SortOrder::default(), SortOrder::Best));
    }

    #[test]
    fn sort_order_server_sort_passthrough_for_best_price_duration() {
        assert!(matches!(SortOrder::Best.server_sort(), SortOrder::Best));
        assert!(matches!(SortOrder::Price.server_sort(), SortOrder::Price));
        assert!(matches!(
            SortOrder::Duration.server_sort(),
            SortOrder::Duration
        ));
    }

    #[test]
    fn sort_order_server_sort_falls_back_to_best_for_time_based() {
        assert!(matches!(
            SortOrder::DepartureTime.server_sort(),
            SortOrder::Best
        ));
        assert!(matches!(
            SortOrder::ArrivalTime.server_sort(),
            SortOrder::Best
        ));
    }

    #[test]
    fn place_type_from_all_known_values() {
        assert!(matches!(PlaceType::from(0), PlaceType::Unspecified));
        assert!(matches!(PlaceType::from(1), PlaceType::Airport));
        assert!(matches!(PlaceType::from(3), PlaceType::MaybeRegion));
        assert!(matches!(PlaceType::from(4), PlaceType::RegionMaybe));
        assert!(matches!(PlaceType::from(5), PlaceType::City));
    }

    #[test]
    fn place_type_from_unknown_falls_back_to_unspecified() {
        assert!(matches!(PlaceType::from(99), PlaceType::Unspecified));
        assert!(matches!(PlaceType::from(-1), PlaceType::Unspecified));
    }

    #[test]
    fn travel_class_from_all_known_values() {
        assert!(matches!(TravelClass::from(1), TravelClass::Economy));
        assert!(matches!(TravelClass::from(2), TravelClass::PremiumEconomy));
        assert!(matches!(TravelClass::from(3), TravelClass::Business));
        assert!(matches!(TravelClass::from(4), TravelClass::First));
    }

    #[test]
    fn travel_class_from_unknown_falls_back_to_economy() {
        assert!(matches!(TravelClass::from(99), TravelClass::Economy));
    }

    #[test]
    fn travel_class_serialize_to_web() {
        assert_eq!(TravelClass::Economy.serialize_to_web().unwrap(), "1");
        assert_eq!(TravelClass::PremiumEconomy.serialize_to_web().unwrap(), "2");
        assert_eq!(TravelClass::Business.serialize_to_web().unwrap(), "3");
        assert_eq!(TravelClass::First.serialize_to_web().unwrap(), "4");
    }

    #[test]
    fn stop_options_from_all_known_values() {
        assert!(matches!(StopOptions::from(0), StopOptions::All));
        assert!(matches!(StopOptions::from(1), StopOptions::NoStop));
        assert!(matches!(StopOptions::from(2), StopOptions::OneOrLess));
        assert!(matches!(StopOptions::from(3), StopOptions::TwoOrLess));
    }

    #[test]
    fn stop_options_from_unknown_falls_back_to_all() {
        assert!(matches!(StopOptions::from(99), StopOptions::All));
    }

    #[test]
    fn stop_options_serialize_to_web() {
        assert_eq!(StopOptions::All.serialize_to_web().unwrap(), "0");
        assert_eq!(StopOptions::NoStop.serialize_to_web().unwrap(), "1");
        assert_eq!(StopOptions::OneOrLess.serialize_to_web().unwrap(), "2");
        assert_eq!(StopOptions::TwoOrLess.serialize_to_web().unwrap(), "3");
    }
}
