use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::SerializeToWeb;

/// Travelers. It contains the number of adults, children, infants on lap and infants in seat.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Travelers {
    pub adults: i32,
    pub children: i32,
    pub infant_on_lap: i32,
    pub infant_in_seat: i32,
}

/// Default is one adult.
impl Default for Travelers {
    fn default() -> Self {
        Self {
            adults: 1,
            children: 0,
            infant_on_lap: 0,
            infant_in_seat: 0,
        }
    }
}

impl Travelers {
    /// Constructs a [`Travelers`] from a 4-element slice
    /// `[adults, children, infant_on_lap, infant_in_seat]`.
    ///
    /// At least one adult is required and the total number of passengers
    /// must not exceed 9 (Google Flights' server-side limit).
    ///
    /// # Errors
    /// Returns an error if the slice does not have exactly 4 elements,
    /// if `adults < 1`, or if the total exceeds 9.
    pub fn new(travellers: Vec<i32>) -> Result<Self> {
        if travellers.len() != 4 {
            return Err(anyhow!(
                "Travelers::new requires exactly 4 elements \
                 [adults, children, infant_on_lap, infant_in_seat], got {}",
                travellers.len()
            ));
        }
        if travellers[0] < 1 {
            return Err(anyhow!("At least one adult traveller is required"));
        }
        let total: i32 = travellers.iter().sum();
        if total > 9 {
            return Err(anyhow!(
                "Total number of passengers cannot exceed 9, got {}",
                total
            ));
        }
        Ok(Self {
            adults: travellers[0],
            children: travellers[1],
            infant_on_lap: travellers[2],
            infant_in_seat: travellers[3],
        })
    }

    /// Conversion to a vector of i32, used in protobuf generation.
    /// It returns a vector of 1, 2, 3, 4 repeated the number of times of the corresponding field.
    pub fn to_proto_vec(&self) -> Vec<i32> {
        let mut travellers = Vec::new();

        travellers.extend(vec![1; self.adults.try_into().unwrap_or(1)]);
        travellers.extend(vec![2; self.children.try_into().unwrap_or(0)]);
        travellers.extend(vec![3; self.infant_in_seat.try_into().unwrap_or(0)]);
        travellers.extend(vec![4; self.infant_on_lap.try_into().unwrap_or(0)]);
        travellers
    }
}

impl SerializeToWeb for Travelers {
    fn serialize_to_web(&self) -> Result<String> {
        Ok(format!(
            r#"[{},{},{},{}]"#,
            self.adults, self.children, self.infant_on_lap, self.infant_in_seat
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::parsers::common::SerializeToWeb;

    use super::*;

    #[test]
    fn travelers_new_valid() {
        let t = Travelers::new(vec![1, 0, 0, 0]).unwrap();
        assert_eq!(t.adults, 1);
    }

    #[test]
    fn travelers_new_wrong_length_errors() {
        assert!(Travelers::new(vec![1, 0, 0]).is_err());
        assert!(Travelers::new(vec![1, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_no_adults_errors() {
        assert!(Travelers::new(vec![0, 1, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_too_many_passengers_errors() {
        assert!(Travelers::new(vec![5, 5, 0, 0]).is_err());
    }

    #[test]
    fn travelers_new_exactly_nine_passes() {
        assert!(Travelers::new(vec![9, 0, 0, 0]).is_ok());
    }

    #[test]
    fn travelers_to_proto_vec_maps_correctly() {
        let t = Travelers::new(vec![2, 1, 1, 1]).unwrap();
        let v = t.to_proto_vec();
        // 2 adults → [1,1], 1 child → [2], 1 infant_in_seat → [3], 1 infant_on_lap → [4]
        assert_eq!(v, vec![1, 1, 2, 3, 4]);
    }

    #[test]
    fn travelers_to_proto_vec_adults_only() {
        let t = Travelers::new(vec![3, 0, 0, 0]).unwrap();
        assert_eq!(t.to_proto_vec(), vec![1, 1, 1]);
    }

    #[test]
    fn travelers_serialize_to_web() {
        let t = Travelers::new(vec![2, 1, 0, 0]).unwrap();
        assert_eq!(t.serialize_to_web().unwrap(), "[2,1,0,0]");
    }
}
