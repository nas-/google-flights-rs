use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::flight_response::RawResponseContainerVec;

use super::{
    common::{decode_inner_object, decode_outer_object},
    flight_response::{CheaperTravelDifferentDates, RawResponseContainer, Unknown0},
};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(transparent)]
pub struct GraphRawResponseContainer {
    graph_respose: Vec<GraphRawResponse>,
}
impl GraphRawResponseContainer {
    pub fn get_all_graphs(&self) -> Vec<CheaperTravelDifferentDates> {
        self.graph_respose
            .clone()
            .into_iter()
            .filter_map(|f| f.price_graph)
            .flatten()
            .collect()
    }
}

impl TryFrom<&str> for GraphRawResponseContainer {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        let outer: Vec<RawResponseContainerVec> = decode_outer_object(value)?;

        let as_before: Vec<Vec<RawResponseContainer>> = outer.into_iter().map(|f| f.resp).collect();

        let res: Vec<String> = as_before
            .iter()
            .flat_map(|f| f.first().ok_or_else(|| anyhow!("Malformed data!")))
            .filter(|f| f.payload.is_some())
            .map(|f| f.payload.as_ref().unwrap().clone())
            .collect();
        let res2: Vec<Result<GraphRawResponse>> =
            res.iter().map(|f| decode_inner_object(f)).collect();
        let res3: Result<Vec<GraphRawResponse>> = res2.into_iter().filter(|f| f.is_ok()).collect();

        Ok(Self {
            graph_respose: res3?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GraphRawResponse {
    unknown0: Unknown0,
    #[serde(default)]
    pub price_graph: Option<Vec<CheaperTravelDifferentDates>>,
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
