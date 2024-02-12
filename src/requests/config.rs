use parsers::common::{
    FlightTimes, Location, StopOptions, StopoverDuration, TotalDuration, TravelClass,
    Travelers,
};
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Config {
    pub departing_date: NaiveDate,
    pub departure: Location,
    pub destination: Location,
    pub stop_options: StopOptions,
    pub travel_class: TravelClass,
    pub return_date: Option<NaiveDate>,
    pub travellers: Travelers,
    pub diff_days: Option<i64>,
    pub date_end_graph: String,
    pub departing_times: FlightTimes,
    pub return_times: FlightTimes,
    pub stopover_max: StopoverDuration,
    pub duration_max: TotalDuration,
    pub is_weekend_trip: bool,
    pub leave_days: Option<Vec<chrono::Weekday>>,
}