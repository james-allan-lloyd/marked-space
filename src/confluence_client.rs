use serde_json::Value;
use std::env;

pub struct ConfluenceClient {
    client: reqwest::blocking::Client,
    api_user: String,
    api_token: String,
    hostname: String,
}

type Result = std::result::Result<reqwest::blocking::Response, reqwest::Error>;

impl ConfluenceClient {
    pub fn new(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").unwrap_or_default(),
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            client: reqwest::blocking::Client::new(),
            hostname: String::from(hostname),
        }
    }

    pub fn get_space_by_key(&self, space_key: &str) -> Result {
        let url = format!("https://{}/wiki/api/v2/spaces", self.hostname);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("keys", space_key)])
            .send()
    }

    pub fn create_page(&self, body_json: Value) -> Result {
        let url = format!("https://{}/wiki/api/v2/pages", self.hostname);
        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .json(&body_json)
            .send()
    }

    pub fn get_page_by_id(&self, page_id: &str) -> Result {
        let url = format!("https://{}/wiki/api/v2/pages/{}", self.hostname, page_id);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("body-format", "storage")])
            .send()
    }

    pub fn get_page_by_title(&self, space_id: &str, title: &str) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/spaces/{}/pages",
            self.hostname, space_id
        );
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("title", title), ("body-format", "storage")])
            .send()
    }

    pub fn update_page(&self, page_id: String, payload: Value) -> Result {
        let url = format!("https://{}/wiki/api/v2/pages/{}", self.hostname, page_id);
        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .json(&payload)
            .send()
    }
}
