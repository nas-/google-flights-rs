//! Configuration types for the `GetExploreDestinations` endpoint.

use crate::parsers::common::{Location, PlaceType, TravelClass, Travelers};
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

/// Resolve an interest name or raw MID to a Knowledge-Graph MID.
///
/// Accepts raw `/m/…` or `/g/…` MIDs (passed through), known names/aliases
/// (resolved via [`mid_from_name`]), and returns an error otherwise. Use this
/// everywhere a user supplies an interest so bad values fail loudly instead of
/// silently returning no results.
pub fn resolve_interest(raw: &str) -> anyhow::Result<String> {
    if raw.starts_with("/m/") || raw.starts_with("/g/") {
        return Ok(raw.to_string());
    }
    if let Some(mid) = mid_from_name(raw) {
        return Ok(mid.to_string());
    }
    let names = known_interest_names().join(", ");
    anyhow::bail!(
        "unknown interest {raw:?}\n\
         Known names: {names}\n\
         Or pass a raw Knowledge-Graph MID, e.g. /m/01rwk"
    )
}

// ---------------------------------------------------------------------------
// Region MID constants
// ---------------------------------------------------------------------------

/// Google Knowledge-Graph MID strings for geographic regions.
///
/// These constants are verified against the Google Flights Explore wire format.
/// For other regions, pass a raw MID directly to `ExploreConfig::destination`.
///
/// To find a region's MID, open Google Flights Explore in a browser, filter
/// to that region, and inspect the `f.req` body (the MID appears as `"/m/…"`
/// with type `6` in the routes array).
pub mod Region {
    #![allow(non_snake_case)]

    /// Northern Europe (UK, Ireland, Scandinavia, Baltic states).
    pub const NORTHERN_EUROPE: &str = "/m/01531v";
    /// The Alps (Switzerland, Austria, northern Italy, southern Germany).
    pub const ALPS: &str = "/m/0lcd";
}

/// Resolve a human-readable region name to a Knowledge-Graph MID.
///
/// Only covers MIDs verified against the Google Flights wire format.
/// For other regions pass a raw `/m/…` MID directly.
///
/// # Examples
/// ```
/// use gflights::requests::config::explore::region_from_name;
/// assert_eq!(region_from_name("alps"), Some("/m/0lcd"));
/// assert_eq!(region_from_name("Northern Europe"), Some("/m/01531v"));
/// // Raw MIDs and unknown names return None.
/// assert_eq!(region_from_name("/m/0lcd"), None);
/// assert_eq!(region_from_name("caribbean"), None);
/// ```
pub fn region_from_name(name: &str) -> Option<&'static str> {
    if name.starts_with("/m/") || name.starts_with("/g/") {
        return None; // raw MID — caller handles passthrough
    }
    let lower = name.to_lowercase();
    const TABLE: &[(&str, &str)] = &[
        // Northern Europe
        ("northern europe", Region::NORTHERN_EUROPE),
        ("scandinavia", Region::NORTHERN_EUROPE),
        ("nordic", Region::NORTHERN_EUROPE),
        // Alps
        ("alps", Region::ALPS),
        ("alpine", Region::ALPS),
    ];
    TABLE
        .iter()
        .find(|(alias, _)| *alias == lower.as_str())
        .map(|(_, mid)| *mid)
}

/// List all known region names (canonical, one per verified MID).
///
/// For other regions pass a raw Knowledge-Graph MID, e.g. `--to /m/01l83z`.
pub fn known_region_names() -> &'static [&'static str] {
    &["northern europe", "alps"]
}

/// Resolve a destination value to a [`Location`] (airport or region).
///
/// Accepts raw region MIDs (`/m/…`, `/g/…` → region type), known region
/// names/aliases (via [`region_from_name`]), and short IATA-looking codes
/// (→ airport type). Region names are checked before the IATA heuristic so a
/// 4-letter alias like `alps` is not misread as an airport code.
pub fn resolve_destination(raw: &str) -> anyhow::Result<Location> {
    if raw.starts_with("/m/") || raw.starts_with("/g/") {
        return Ok(Location {
            loc_identifier: raw.to_string(),
            loc_type: PlaceType::Region,
            location_name: None,
        });
    }
    if let Some(mid) = region_from_name(raw) {
        return Ok(Location {
            loc_identifier: mid.to_string(),
            loc_type: PlaceType::Region,
            location_name: Some(raw.to_string()),
        });
    }
    if raw.len() <= 4 && raw.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(Location {
            loc_identifier: raw.to_uppercase(),
            loc_type: PlaceType::Airport,
            location_name: None,
        });
    }
    let regions = known_region_names().join(", ");
    anyhow::bail!(
        "unknown destination {raw:?}\n\
         Use an IATA airport code (e.g. BCN), a region name ({regions}),\n\
         or a raw Knowledge-Graph MID (e.g. /m/01531v)"
    )
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
    /// IATA code of the nearest airport to the destination (geographic label).
    pub nearest_airport: String,
    /// IATA code of the airport the priced flight actually lands at.
    ///
    /// For multi-airport cities this often differs from [`Self::nearest_airport`]:
    /// e.g. Verdon Gorge has `nearest_airport = "NCE"` but cheap Ryanair flights
    /// land at `flight_airport = "MRS"`.  Prefer this field when booking or when
    /// showing the user which airport to fly into.
    pub flight_airport: Option<String>,
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
