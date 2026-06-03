//! Configuration types for the `GetExploreDestinations` endpoint.

use crate::parsers::common::{Location, TravelClass, Travelers};
use crate::requests::config::Currency;

// ---------------------------------------------------------------------------
// Duration / date options
// ---------------------------------------------------------------------------

/// How long the trip should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreDuration {
    /// A weekend trip (Saturday + Sunday).
    Weekend,
    /// A trip of approximately one week (7 days).
    OneWeek,
    /// A trip of approximately two weeks (14 days).
    TwoWeeks,
}

impl ExploreDuration {
    /// Wire format code: 1=weekend, 2=1week, 3=2weeks.
    pub fn as_wire_code(self) -> i32 {
        match self {
            ExploreDuration::Weekend => 1,
            ExploreDuration::OneWeek => 2,
            ExploreDuration::TwoWeeks => 3,
        }
    }
}

/// A calendar month (1–12) for filtering explore results.
#[derive(Debug, Clone, Copy)]
pub struct ExploreDate {
    /// Month number, 1 = January … 12 = December.
    pub month: u8,
}

// ---------------------------------------------------------------------------
// Map bounds
// ---------------------------------------------------------------------------

/// Geographic bounding box for the explore map view.
#[derive(Debug, Clone, Copy)]
pub struct MapBounds {
    /// South-west corner `(lat, lng)`.
    pub sw: (f64, f64),
    /// North-east corner `(lat, lng)`.
    pub ne: (f64, f64),
}

// ---------------------------------------------------------------------------
// Interest MID constants
// ---------------------------------------------------------------------------

/// Google Knowledge-Graph MID strings for interest categories.
///
/// Pass one of these as `ExploreConfig::interest` to filter explore results
/// to destinations matching that category.
pub mod Interest {
    #![allow(non_snake_case)]

    /// Outdoors / nature.
    pub const OUTDOORS: &str = "/g/11bc58l13w";
    /// Beaches.
    pub const BEACHES: &str = "/m/0b3yr";
    /// Museums.
    pub const MUSEUMS: &str = "/m/09cmq";
    /// History & culture.
    pub const HISTORY: &str = "/m/03g3w";
    /// Skiing.
    pub const SKIING: &str = "/m/071k0";
    /// Rock climbing.
    pub const CLIMBING: &str = "/m/01rwk";
}

/// Resolve a human-readable interest name to a Knowledge-Graph MID.
///
/// Accepts canonical names and common aliases (case-insensitive).
/// Returns `None` when the name is not recognised — callers should suggest
/// using a raw `/m/…` or `/g/…` MID instead.
///
/// # Examples
/// ```
/// use gflights::requests::config::explore::mid_from_name;
/// assert_eq!(mid_from_name("beaches"), Some("/m/0b3yr"));
/// assert_eq!(mid_from_name("Rock Climbing"), Some("/m/01rwk"));
/// // Raw MIDs return None — the CLI layer passes them through directly.
/// assert_eq!(mid_from_name("/m/0b3yr"), None);
/// assert_eq!(mid_from_name("surfing"), None);
/// ```
pub fn mid_from_name(name: &str) -> Option<&'static str> {
    // Raw MID passthrough.
    if name.starts_with("/m/") || name.starts_with("/g/") {
        // We can't return name directly (wrong lifetime), so search the table.
        // If not in table, the caller already has a raw MID — return it as-is
        // by leaking is not ideal; instead document that raw MIDs bypass lookup.
        // We handle this in the CLI layer instead (see cmd_explore).
        return None; // sentinel: CLI handles raw MIDs before calling this fn
    }

    let lower = name.to_lowercase();
    // Static table: (alias, MID).  Multiple rows per MID = multiple aliases.
    const TABLE: &[(&str, &str)] = &[
        // Outdoors
        ("outdoors", Interest::OUTDOORS),
        ("nature", Interest::OUTDOORS),
        ("outdoor", Interest::OUTDOORS),
        // Beaches
        ("beaches", Interest::BEACHES),
        ("beach", Interest::BEACHES),
        ("coast", Interest::BEACHES),
        ("coastal", Interest::BEACHES),
        // Museums
        ("museums", Interest::MUSEUMS),
        ("museum", Interest::MUSEUMS),
        ("art", Interest::MUSEUMS),
        // History
        ("history", Interest::HISTORY),
        ("culture", Interest::HISTORY),
        ("historical", Interest::HISTORY),
        ("heritage", Interest::HISTORY),
        // Skiing
        ("skiing", Interest::SKIING),
        ("ski", Interest::SKIING),
        ("snowboarding", Interest::SKIING),
        ("snow", Interest::SKIING),
        // Climbing
        ("climbing", Interest::CLIMBING),
        ("rock climbing", Interest::CLIMBING),
        ("bouldering", Interest::CLIMBING),
        ("mountaineering", Interest::CLIMBING),
    ];

    TABLE
        .iter()
        .find(|(alias, _)| *alias == lower.as_str())
        .map(|(_, mid)| *mid)
}

/// List all known interest names (canonical, one per MID).
pub fn known_interest_names() -> &'static [&'static str] {
    &[
        "outdoors", "beaches", "museums", "history", "skiing", "climbing",
    ]
}

// ---------------------------------------------------------------------------
// Region MID constants
// ---------------------------------------------------------------------------

/// Google Knowledge-Graph MID strings for geographic regions.
///
/// Pass one of these as `ExploreConfig::destination` (with `PlaceType::Region`)
/// to filter explore results to destinations within that region.
pub mod Region {
    #![allow(non_snake_case)]

    /// Northern Europe.
    pub const NORTHERN_EUROPE: &str = "/m/01531v";
    /// Southern Europe.
    pub const SOUTHERN_EUROPE: &str = "/m/048_b";
    /// Western Europe.
    pub const WESTERN_EUROPE: &str = "/m/04_1l";
    /// Eastern Europe.
    pub const EASTERN_EUROPE: &str = "/m/05lrn";
    /// The Alps.
    pub const ALPS: &str = "/m/0lcd";
    /// Mediterranean.
    pub const MEDITERRANEAN: &str = "/m/04vlnn";
    /// Southeast Asia.
    pub const SOUTHEAST_ASIA: &str = "/m/07bxq";
    /// East Asia.
    pub const EAST_ASIA: &str = "/m/011yph";
    /// North America.
    pub const NORTH_AMERICA: &str = "/m/05sb1";
    /// Caribbean.
    pub const CARIBBEAN: &str = "/m/01l83z";
    /// Central America.
    pub const CENTRAL_AMERICA: &str = "/m/06yfb";
    /// South America.
    pub const SOUTH_AMERICA: &str = "/m/015fr";
    /// Africa.
    pub const AFRICA: &str = "/m/0dg3n1";
    /// Middle East.
    pub const MIDDLE_EAST: &str = "/m/01n7";
}

/// Resolve a human-readable region name to a Knowledge-Graph MID.
///
/// Accepts canonical names and common aliases (case-insensitive).
/// Returns `None` when the name is not recognised — callers should suggest
/// using a raw `/m/…` or `/g/…` MID or an IATA airport code instead.
///
/// # Examples
/// ```
/// use gflights::requests::config::explore::region_from_name;
/// assert_eq!(region_from_name("alps"), Some("/m/0lcd"));
/// assert_eq!(region_from_name("Northern Europe"), Some("/m/01531v"));
/// // Raw MIDs and unknown names return None.
/// assert_eq!(region_from_name("/m/0lcd"), None);
/// assert_eq!(region_from_name("surfing"), None);
/// ```
pub fn region_from_name(name: &str) -> Option<&'static str> {
    if name.starts_with("/m/") || name.starts_with("/g/") {
        return None; // raw MID — caller handles passthrough
    }
    let lower = name.to_lowercase();
    const TABLE: &[(&str, &str)] = &[
        ("northern europe", Region::NORTHERN_EUROPE),
        ("scandinavia", Region::NORTHERN_EUROPE),
        ("nordic", Region::NORTHERN_EUROPE),
        ("southern europe", Region::SOUTHERN_EUROPE),
        ("western europe", Region::WESTERN_EUROPE),
        ("eastern europe", Region::EASTERN_EUROPE),
        ("alps", Region::ALPS),
        ("alpine", Region::ALPS),
        ("mediterranean", Region::MEDITERRANEAN),
        ("med", Region::MEDITERRANEAN),
        ("southeast asia", Region::SOUTHEAST_ASIA),
        ("sea", Region::SOUTHEAST_ASIA),
        ("east asia", Region::EAST_ASIA),
        ("north america", Region::NORTH_AMERICA),
        ("caribbean", Region::CARIBBEAN),
        ("central america", Region::CENTRAL_AMERICA),
        ("south america", Region::SOUTH_AMERICA),
        ("latin america", Region::SOUTH_AMERICA),
        ("africa", Region::AFRICA),
        ("middle east", Region::MIDDLE_EAST),
    ];
    TABLE
        .iter()
        .find(|(alias, _)| *alias == lower.as_str())
        .map(|(_, mid)| *mid)
}

/// List all known region names (canonical, one per MID).
pub fn known_region_names() -> &'static [&'static str] {
    &[
        "northern europe",
        "southern europe",
        "western europe",
        "eastern europe",
        "alps",
        "mediterranean",
        "southeast asia",
        "east asia",
        "north america",
        "caribbean",
        "central america",
        "south america",
        "africa",
        "middle east",
    ]
}

// ---------------------------------------------------------------------------
// Main config struct
// ---------------------------------------------------------------------------

/// Configuration for an `GetExploreDestinations` request.
///
/// Build directly or use struct-literal syntax; all fields except `origin`
/// and `travellers` have sensible defaults via `Default`.
#[derive(Debug, Clone)]
pub struct ExploreConfig {
    /// One or more origin airports / cities.
    pub origin: Vec<Location>,

    /// Optional destination filter: restrict results to a specific airport or
    /// geographic region.
    ///
    /// - Airport IATA code → use `PlaceType::Airport` (type 0)
    /// - Region MID (e.g. `"/m/01531v"`) → use `PlaceType::Region` (type 6)
    ///
    /// Use constants from [`Region`] or raw MIDs from the Knowledge Graph.
    pub destination: Option<Location>,

    /// Optional calendar month to filter results (1–12).
    pub trip_date: Option<ExploreDate>,

    /// Trip duration: weekend / 1-week / 2-weeks.
    pub trip_duration: ExploreDuration,

    /// Maximum total ticket price (both ways) in the configured currency.
    pub max_price: Option<i32>,

    /// Google Knowledge-Graph MID for an interest category.
    /// Use constants from [`Interest`].
    pub interest: Option<String>,

    /// Restrict to a single airline alliance.
    pub airline_alliance: Option<crate::parsers::common::Alliance>,

    /// Maximum one-way flight duration in minutes.
    pub max_flight_duration_minutes: Option<u32>,

    /// Baggage allowance: `(carry_on_count, checked_count)`.
    pub baggage: Option<(u8, u8)>,

    /// Optional map bounding box (SW and NE corners).
    pub map_bounds: Option<MapBounds>,

    /// Traveller counts.
    pub travellers: Travelers,

    /// Cabin class.
    pub travel_class: TravelClass,

    /// Currency for prices.
    pub currency: Currency,

    /// BCP-47 language subtag, e.g. `"en"`, `"fr"`.
    pub language: String,

    /// ISO 3166-1 alpha-2 country code, e.g. `"GB"`.
    pub country: String,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            origin: Vec::new(),
            destination: None,
            trip_date: None,
            trip_duration: ExploreDuration::OneWeek,
            max_price: None,
            interest: None,
            airline_alliance: None,
            max_flight_duration_minutes: None,
            baggage: None,
            map_bounds: None,
            travellers: Travelers::default(),
            travel_class: TravelClass::Economy,
            currency: Currency::default(),
            language: "en".to_string(),
            country: "GB".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// One destination returned by `GetExploreDestinations`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExploreResult {
    /// Google place ID (e.g. `"/m/0vzm"` for Vienna).
    pub place_id: String,
    /// English name of the destination.
    pub name: String,
    /// Country name.
    pub country: String,
    /// `(lat, lng)` coordinates of the destination.
    pub coords: (f64, f64),
    /// URL of a cover photo (if available).
    pub image_url: Option<String>,
    /// IATA code of the nearest airport.
    pub nearest_airport: String,
    /// Earliest available outbound departure date.
    pub date_from: Option<chrono::NaiveDate>,
    /// Latest available return date (round-trip) or arrival date (one-way).
    pub date_to: Option<chrono::NaiveDate>,
    /// Cheapest round-trip flight price (both legs combined).
    pub price: Option<i32>,
    /// Primary operating airline code.
    pub airline: Option<String>,
    /// Number of stops on the outbound leg.
    pub stops: Option<u8>,
    /// Total outbound flight duration in minutes.
    pub flight_duration_minutes: Option<u32>,
    /// Nightly accommodation price at the destination.
    pub accommodation_price: Option<i32>,
    /// Opaque booking token for constructing a deep link.
    pub booking_token: String,
}
