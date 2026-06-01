use anyhow::Result;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Airline / Alliance filter types
// ---------------------------------------------------------------------------

/// One of the three major IATA global alliances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alliance {
    OneWorld,
    SkyTeam,
    StarAlliance,
}

impl Alliance {
    /// The string Google Flights uses for this alliance in the request body.
    pub fn as_google_str(self) -> &'static str {
        match self {
            Alliance::OneWorld => "ONEWORLD",
            Alliance::SkyTeam => "SKYTEAM",
            Alliance::StarAlliance => "STAR_ALLIANCE",
        }
    }
}

/// A validated two-letter IATA airline code (e.g. `"LX"`, `"LH"`, `"BA"`).
///
/// The code is normalised to upper-case on construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirlineCode(String);

impl AirlineCode {
    /// Validates and creates an [`AirlineCode`].
    ///
    /// # Errors
    /// Returns an error if `code` is not exactly 2 ASCII letters.
    pub fn new(code: impl Into<String>) -> Result<Self> {
        let s = code.into().to_uppercase();
        anyhow::ensure!(
            s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()),
            "IATA airline code must be exactly 2 ASCII letters, got {:?}",
            s
        );
        Ok(AirlineCode(s))
    }

    /// Returns the code as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AirlineCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An airline filter entry: either a specific carrier or an entire alliance.
///
/// Both include and exclude filters in `Config` use `Vec<AirlineFilter>`.
/// The list is serialised as a flat JSON array of strings —
/// IATA codes (`"LX"`) and alliance names (`"ONEWORLD"`) mixed together —
/// which is exactly what the Google Flights wire format expects at indices
/// \[4\] (include) and \[5\] (exclude) of the per-leg array.
///
/// # Parsing from a string
///
/// `AirlineFilter` implements [`std::str::FromStr`]:
/// - Alliance names (`ONEWORLD`, `SKYTEAM`, `STAR_ALLIANCE`) are case-insensitive.
/// - Everything else is validated as a 2-letter IATA code.
///
/// This means clap parses `--airline LX` and `--airline ONEWORLD` automatically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AirlineFilter {
    Airline(AirlineCode),
    Alliance(Alliance),
}

impl AirlineFilter {
    /// The string Google Flights uses for this filter in the request body.
    pub fn as_google_str(&self) -> &str {
        match self {
            AirlineFilter::Airline(c) => c.as_str(),
            AirlineFilter::Alliance(a) => a.as_google_str(),
        }
    }
}

impl std::str::FromStr for AirlineFilter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "ONEWORLD" => Ok(AirlineFilter::Alliance(Alliance::OneWorld)),
            "SKYTEAM" => Ok(AirlineFilter::Alliance(Alliance::SkyTeam)),
            "STAR_ALLIANCE" => Ok(AirlineFilter::Alliance(Alliance::StarAlliance)),
            _ => Ok(AirlineFilter::Airline(AirlineCode::new(s)?)),
        }
    }
}

impl std::fmt::Display for AirlineFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_google_str())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn airline_code_valid_lowercase_is_upcased() {
        let code = AirlineCode::new("lx").unwrap();
        assert_eq!(code.as_str(), "LX");
        assert_eq!(code.to_string(), "LX");
    }

    #[test]
    fn airline_code_valid_uppercase_accepted() {
        let code = AirlineCode::new("BA").unwrap();
        assert_eq!(code.as_str(), "BA");
    }

    #[test]
    fn airline_code_too_short_errors() {
        assert!(AirlineCode::new("A").is_err());
    }

    #[test]
    fn airline_code_too_long_errors() {
        assert!(AirlineCode::new("LHR").is_err());
    }

    #[test]
    fn airline_code_digits_error() {
        assert!(AirlineCode::new("B6").is_err(), "digits should be rejected");
    }

    #[test]
    fn airline_code_empty_errors() {
        assert!(AirlineCode::new("").is_err());
    }

    #[test]
    fn alliance_as_google_str_all_variants() {
        assert_eq!(Alliance::OneWorld.as_google_str(), "ONEWORLD");
        assert_eq!(Alliance::SkyTeam.as_google_str(), "SKYTEAM");
        assert_eq!(Alliance::StarAlliance.as_google_str(), "STAR_ALLIANCE");
    }

    #[test]
    fn airline_filter_from_str_oneworld_case_insensitive() {
        let f: AirlineFilter = "oneworld".parse().unwrap();
        assert_eq!(f, AirlineFilter::Alliance(Alliance::OneWorld));
        assert_eq!(f.as_google_str(), "ONEWORLD");
        assert_eq!(f.to_string(), "ONEWORLD");
    }

    #[test]
    fn airline_filter_from_str_skyteam() {
        let f: AirlineFilter = "SKYTEAM".parse().unwrap();
        assert_eq!(f, AirlineFilter::Alliance(Alliance::SkyTeam));
    }

    #[test]
    fn airline_filter_from_str_star_alliance() {
        let f: AirlineFilter = "star_alliance".parse().unwrap();
        assert_eq!(f, AirlineFilter::Alliance(Alliance::StarAlliance));
    }

    #[test]
    fn airline_filter_from_str_iata_code() {
        let f: AirlineFilter = "LH".parse().unwrap();
        assert!(matches!(f, AirlineFilter::Airline(_)));
        assert_eq!(f.as_google_str(), "LH");
        assert_eq!(f.to_string(), "LH");
    }

    #[test]
    fn airline_filter_from_str_invalid_code_errors() {
        assert!("LHR".parse::<AirlineFilter>().is_err());
    }

    #[test]
    fn airline_filter_alliance_as_google_str() {
        let f = AirlineFilter::Alliance(Alliance::StarAlliance);
        assert_eq!(f.as_google_str(), "STAR_ALLIANCE");
    }
}
