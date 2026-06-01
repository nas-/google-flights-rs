use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::SerializeToWeb;

/// Stop over duration. It can be a number of minutes or unlimited, with default unlimited.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default)]
pub enum StopoverDuration {
    Minutes(u32),
    #[default]
    UNLIMITED,
}

impl StopoverDuration {
    pub fn to_option(&self) -> Option<u32> {
        match *self {
            Self::Minutes(i) => Some(i),
            Self::UNLIMITED => None,
        }
    }

    pub fn to_i32(&self) -> Option<i32> {
        match *self {
            Self::Minutes(i) => Some(i as i32),
            Self::UNLIMITED => None,
        }
    }
}

impl SerializeToWeb for StopoverDuration {
    // Google ui allow stopover max to be checked in 30 mins intervals.
    fn serialize_to_web(&self) -> Result<String> {
        match self {
            StopoverDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return Ok(format!("{}", mins.div_ceil(30) * 30));
                }
                Ok(format!("{mins}"))
            }
            StopoverDuration::UNLIMITED => Ok("null".to_string()),
        }
    }
}

/// Total duration. It can be a number of minutes or unlimited, with default unlimited.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub enum TotalDuration {
    Minutes(u32),
    #[default]
    UNLIMITED,
}

impl TotalDuration {
    pub fn to_option(&self) -> Option<u32> {
        match *self {
            Self::Minutes(i) => Some(i),
            Self::UNLIMITED => None,
        }
    }
}

impl SerializeToWeb for TotalDuration {
    // Google ui allow total_length max to be checked in 30 mins intervals.
    // total_length is within parentheses
    fn serialize_to_web(&self) -> Result<String> {
        match self {
            TotalDuration::Minutes(mins) => {
                if mins % 30 != 0 {
                    return Ok(format!("[{}]", mins.div_ceil(30) * 30));
                }
                Ok(format!("[{mins}]"))
            }
            TotalDuration::UNLIMITED => Ok("null".to_string()),
        }
    }
}

/// Flight times filters. It is the departure hours, and the arrival hours.
///
/// Example: `[0,23,13,23]` → leave between 0:00 and 23:59, arrive between 13:00 and 23:59.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FlightTimes {
    departure_hour_min: Option<u32>,
    departure_hour_max: Option<u32>,
    arrival_hour_min: Option<u32>,
    arrival_hour_max: Option<u32>,
}

impl FlightTimes {
    pub fn new(
        departure_hour_min: u32,
        departure_hour_max: u32,
        arrival_hour_min: u32,
        arrival_hour_max: u32,
    ) -> Self {
        let min_hour_departure = match departure_hour_min {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let max_departure_hour = match departure_hour_max {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let min_hour_arrival = match arrival_hour_min {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };
        let max_hour_arrival = match arrival_hour_max {
            x if x < 24 && x > 0 => Some(x),
            _ => None,
        };

        Self {
            departure_hour_min: min_hour_departure,
            departure_hour_max: max_departure_hour,
            arrival_hour_min: min_hour_arrival,
            arrival_hour_max: max_hour_arrival,
        }
    }

    pub fn get_departure_hour_min(&self) -> Option<u32> {
        self.departure_hour_min
    }

    pub fn get_departure_hour_max(&self) -> Option<u32> {
        self.departure_hour_max
    }

    pub fn get_arrival_hour_min(&self) -> Option<u32> {
        self.arrival_hour_min
    }

    pub fn get_arrival_hour_max(&self) -> Option<u32> {
        self.arrival_hour_max
    }
}

impl SerializeToWeb for FlightTimes {
    fn serialize_to_web(&self) -> Result<String> {
        if self.departure_hour_min.is_none()
            && self.departure_hour_max.is_none()
            && self.arrival_hour_min.is_none()
            && self.arrival_hour_max.is_none()
        {
            Ok("null".to_string())
        } else {
            Ok(format!(
                "[{},{},{},{}]",
                self.departure_hour_min.unwrap_or(0),
                self.departure_hour_max.unwrap_or(23),
                self.arrival_hour_min.unwrap_or(0),
                self.arrival_hour_max.unwrap_or(23)
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::parsers::common::SerializeToWeb;

    use super::*;

    #[test]
    fn stopover_duration_serialize_exact_multiple_of_30() {
        assert_eq!(
            StopoverDuration::Minutes(60).serialize_to_web().unwrap(),
            "60"
        );
        assert_eq!(
            StopoverDuration::Minutes(120).serialize_to_web().unwrap(),
            "120"
        );
    }

    #[test]
    fn stopover_duration_serialize_rounds_up_to_nearest_30() {
        assert_eq!(
            StopoverDuration::Minutes(45).serialize_to_web().unwrap(),
            "60"
        );
        assert_eq!(
            StopoverDuration::Minutes(31).serialize_to_web().unwrap(),
            "60"
        );
        assert_eq!(
            StopoverDuration::Minutes(1).serialize_to_web().unwrap(),
            "30"
        );
    }

    #[test]
    fn stopover_duration_unlimited_serializes_to_null() {
        assert_eq!(
            StopoverDuration::UNLIMITED.serialize_to_web().unwrap(),
            "null"
        );
    }

    #[test]
    fn stopover_duration_to_option() {
        assert_eq!(StopoverDuration::Minutes(90).to_option(), Some(90));
        assert_eq!(StopoverDuration::UNLIMITED.to_option(), None);
    }

    #[test]
    fn stopover_duration_to_i32() {
        assert_eq!(StopoverDuration::Minutes(60).to_i32(), Some(60));
        assert_eq!(StopoverDuration::UNLIMITED.to_i32(), None);
    }

    #[test]
    fn total_duration_serialize_exact_multiple_of_30() {
        assert_eq!(
            TotalDuration::Minutes(180).serialize_to_web().unwrap(),
            "[180]"
        );
    }

    #[test]
    fn total_duration_serialize_rounds_up_to_nearest_30() {
        assert_eq!(
            TotalDuration::Minutes(91).serialize_to_web().unwrap(),
            "[120]"
        );
    }

    #[test]
    fn total_duration_unlimited_serializes_to_null() {
        assert_eq!(TotalDuration::UNLIMITED.serialize_to_web().unwrap(), "null");
    }

    #[test]
    fn total_duration_to_option() {
        assert_eq!(TotalDuration::Minutes(200).to_option(), Some(200));
        assert_eq!(TotalDuration::UNLIMITED.to_option(), None);
    }

    #[test]
    fn flight_times_default_serializes_to_null() {
        let ft = FlightTimes::default();
        assert_eq!(ft.serialize_to_web().unwrap(), "null");
    }

    #[test]
    fn flight_times_with_values_serializes_correctly() {
        let ft = FlightTimes::new(6, 22, 8, 20);
        assert_eq!(ft.serialize_to_web().unwrap(), "[6,22,8,20]");
    }

    #[test]
    fn flight_times_zero_hours_treated_as_none() {
        let ft = FlightTimes::new(0, 0, 0, 0);
        assert_eq!(ft.serialize_to_web().unwrap(), "null");
    }

    #[test]
    fn flight_times_out_of_range_hours_treated_as_none() {
        let ft = FlightTimes::new(25, 25, 25, 25);
        assert_eq!(ft.serialize_to_web().unwrap(), "null");
    }

    #[test]
    fn flight_times_getters_return_correct_values() {
        let ft = FlightTimes::new(7, 21, 9, 18);
        assert_eq!(ft.get_departure_hour_min(), Some(7));
        assert_eq!(ft.get_departure_hour_max(), Some(21));
        assert_eq!(ft.get_arrival_hour_min(), Some(9));
        assert_eq!(ft.get_arrival_hour_max(), Some(18));
    }

    #[test]
    fn flight_times_getters_return_none_for_default() {
        let ft = FlightTimes::default();
        assert_eq!(ft.get_departure_hour_min(), None);
        assert_eq!(ft.get_departure_hour_max(), None);
        assert_eq!(ft.get_arrival_hour_min(), None);
        assert_eq!(ft.get_arrival_hour_max(), None);
    }
}
