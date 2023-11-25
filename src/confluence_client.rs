use serde_json::Value;
use std::env;

pub struct ConfluenceClient {
    client: reqwest::blocking::Client,
    api_user: String,
    api_token: String,
    hostname: String,
}

impl ConfluenceClient {
    pub fn new(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").expect("API_USER not set"),
            api_token: env::var("API_TOKEN").expect("API_TOKEN not set"),
            client: reqwest::blocking::Client::new(),
            hostname: String::from(hostname),
        }
    }

    pub fn get_space_by_key(
        &self,
        space_key: &str,
    ) -> std::result::Result<reqwest::blocking::Response, reqwest::Error> {
        let url = format!("https://{}/wiki/api/v2/spaces", self.hostname);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("keys", space_key)])
            .send()
    }

    pub fn create_page(
        &self,
        body_json: Value,
    ) -> std::result::Result<reqwest::blocking::Response, reqwest::Error> {
        let url = format!("https://{}/wiki/api/v2/pages", self.hostname);
        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .json(&body_json)
            .send()
    }
}
