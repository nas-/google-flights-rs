use percent_encoding::utf8_percent_encode;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::common::{RequestBody, ToRequestBody, CHARACTERS_TO_ENCODE};

use anyhow::Result;

#[derive(Debug, Deserialize, Serialize)]
pub struct CityRequestOptions {
    city: String,
    frontend_version: String,
}

impl CityRequestOptions {
    pub fn new(city: &str, frontend_version: &str) -> Self {
        Self {
            city: city.into(),
            frontend_version: frontend_version.into(),
        }
    }
}

impl ToRequestBody for CityRequestOptions {
    fn to_request_body(&self) -> Result<RequestBody> {
        self.try_into()
    }
}

impl TryFrom<&CityRequestOptions> for RequestBody {
    type Error = anyhow::Error;
    fn try_from(options: &CityRequestOptions) -> Result<Self> {
        let epoch_now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

        let body = format!(
            r#"f.req=[[["H028ib","[\"{0}\",[1,2,3,5,4],null,[1,1,1],1]",null,"generic"]]]&at=AAuQa1qqZgn5F209lkOLZp20vq5d:{1}&"#,
            &options.city, epoch_now
        );
        let url = format!("https://www.google.com/_/TravelFrontendUi/data/batchexecute?rpcids=H028ib&source-path=/travel/flights&f.sid=-2414068248310847860&bl={}&hl=en-GB&soc-app=162&soc-platform=1&soc-device=1&_reqid=581503&rt=c",options.frontend_version);
        Ok(Self {
            url,
            body: utf8_percent_encode(&body, CHARACTERS_TO_ENCODE).to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Ok;

    use super::*;
    #[test]
    fn test_produce_correct_body() -> Result<()> {
        let frontend_version = "boq_travel-frontend-ui_20240110.02_p0".to_string();
        let options = CityRequestOptions {
            city: "london".to_string(),
            frontend_version,
        };
        let req: RequestBody = (&options).try_into()?;
        let expected = "f.req=%5B%5B%5B%22H028ib%22%2C%22%5B%5C%22london%5C%22%2C%5B1%2C2%2C3%2C5%2C4%5D%2Cnull%2C%5B1%2C1%2C1%5D%2C1%5D%22%2Cnull%2C%22generic%22%5D%5D%5D&at=AAuQa1qqZgn5F209lkOLZp20vq5d%3A";
        assert!(req.body.starts_with(expected));

        Ok(())
    }
}
