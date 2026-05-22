use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::parsers::common::get_idx;
use crate::parsers::flight_response::RawResponseContainerVec;

use super::{
    common::{decode_inner_object, decode_outer_object},
    flight_response::{CheaperTravelDifferentDates, RawResponseContainer},
};

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
}
