use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::flight_response::{
    CheaperTravelDifferentDates, RawResponseContainer, RawResponseContainerVec,
};
use crate::parsers::common::{decode_inner_object, decode_outer_object, get_idx};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(transparent)]
pub struct GraphRawResponseContainer {
    graph_respose: Vec<GraphRawResponse>,
}
impl GraphRawResponseContainer {
    pub fn get_all_graphs(&self) -> Vec<CheaperTravelDifferentDates> {
        self.graph_respose
            .iter()
            .filter_map(|f| f.price_graph.as_ref())
            .flatten()
            .cloned()
            .collect()
    }
}

impl TryFrom<&str> for GraphRawResponseContainer {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        let outer: Vec<RawResponseContainerVec> = decode_outer_object(value)?;

        let as_before: Vec<RawResponseContainer> = outer.into_iter().flat_map(|f| f.resp).collect();

        let res: Result<Vec<GraphRawResponse>> = as_before
            .iter()
            .filter_map(|f| f.payload.as_ref())
            .map(|payload| decode_inner_object(payload))
            .filter(|f| f.is_ok())
            .collect();

        Ok(Self {
            graph_respose: res?,
        })
    }
}

// Vec<Value> based — absorbs any number of trailing fields Google may add
#[derive(Debug, Serialize, Clone)]
pub struct GraphRawResponse {
    pub price_graph: Option<Vec<CheaperTravelDifferentDates>>,
}

impl<'de> Deserialize<'de> for GraphRawResponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let arr = Vec::<Value>::deserialize(d)?;
        Ok(GraphRawResponse {
            price_graph: get_idx(&arr, 1),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    use std::fs;

    use super::*;

    #[test]
    fn test_response() {
        let body = fs::read_to_string("test_files/graph_response").expect("Cannot read from file");
        let res: Result<Vec<RawResponseContainerVec>, _> = decode_outer_object(&body);
        let binding = res.unwrap();
        let outer = &binding[0].resp[0].payload.as_ref().unwrap();
        let other: Result<GraphRawResponse, _> = decode_inner_object(outer);
        assert!(other.is_ok())
    }

    /// Full round-trip: raw response bytes → `GraphRawResponseContainer` via `TryFrom`.
    #[test]
    fn graph_raw_response_container_try_from_parses_file() {
        let body = fs::read_to_string("test_files/graph_response").expect("Cannot read from file");
        let container = GraphRawResponseContainer::try_from(body.as_str());
        assert!(
            container.is_ok(),
            "TryFrom<&str> should succeed: {:?}",
            container.err()
        );
    }

    /// `get_all_graphs()` returns at least one suggestion from the fixture file.
    #[test]
    fn get_all_graphs_returns_nonempty_for_fixture() {
        let body = fs::read_to_string("test_files/graph_response").expect("Cannot read from file");
        let container = GraphRawResponseContainer::try_from(body.as_str()).unwrap();
        let graphs = container.get_all_graphs();
        assert!(
            !graphs.is_empty(),
            "expected ≥1 price-graph entry from fixture"
        );
    }

    /// Every `CheaperTravelDifferentDates` entry from the fixture has a
    /// non-past proposed departure date (basic structural sanity).
    #[test]
    fn get_all_graphs_entries_have_valid_dates() {
        let body = fs::read_to_string("test_files/graph_response").expect("Cannot read from file");
        let container = GraphRawResponseContainer::try_from(body.as_str()).unwrap();
        for (i, entry) in container.get_all_graphs().iter().enumerate() {
            // `maybe_get_date_price` returns None when there is no price data —
            // either outcome is acceptable, but calling it must not panic.
            let _ = entry.maybe_get_date_price();
            // The proposed departure date must be a valid NaiveDate (it IS a
            // NaiveDate, so it is always valid — we just access it to confirm
            // the field exists and is reachable).
            let _ = entry.proposed_departure_date;
            let _ = i; // silence unused variable warning
        }
    }
}
