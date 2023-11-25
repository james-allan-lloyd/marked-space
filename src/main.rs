use std::env;
use std::fmt;

use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

mod confluence_client;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Space {
    _id: String,
    _key: String,
    _name: String,
    homepage_id: String,
}

#[derive(Debug, Clone)]
struct ConfluenceError {
    message: String,
}

impl std::error::Error for ConfluenceError {}

impl fmt::Display for ConfluenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error {}", self.message.as_str())
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn get_space(confluence_client: &ConfluenceClient, space_id: &str) -> Result<Space> {
    let resp = match confluence_client.get_space_by_key(space_id) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(ConfluenceError {
                message: String::from("Failed to get space id"),
            }
            .into());
        }
    };

    if !resp.status().is_success() {
        return Err(ConfluenceError {
            message: format!(
                "Failed: {}",
                resp.text().unwrap_or("no text content".into())
            ),
        }
        .into());
    }
    let json = resp.json::<serde_json::Value>()?;

    match serde_json::from_value::<Space>(json["results"][0].clone()) {
        Ok(parsed_space) => return Ok(parsed_space),
        Err(_) => {
            return Err(ConfluenceError {
                message: "Failed to parse response.".into(),
            }
            .into())
        }
    };
}

struct Page {
    title: String,
    content: String,
}

fn create_page(confluence_client: &ConfluenceClient, space: Space, page: Page) -> Result<()> {
    let payload = json!({
        "spaceId": space._id,
        "status": "current",
        "title": page.title,
        "parentId": space.homepage_id,
        "body": {
            "representation": "storage",
            "value": page.content
        }
    });

    let resp = match confluence_client.create_page(payload) {
        Ok(r) => r,
        Err(_) => {
            return Err(ConfluenceError {
                message: "Failed to create page".into(),
            }
            .into())
        }
    };

    let status = resp.status();
    let content = resp.text().unwrap_or_default();
    let json = match serde_json::from_str::<serde_json::Value>(content.as_str()) {
        Ok(j) => j,
        Err(_) => todo!(),
    };

    if status == StatusCode::BAD_REQUEST {
        println!("Updating page");
        println!("{:#?}", json);
    } else if !status.is_success() {
        return Err(ConfluenceError {
            message: format!("Failed to create page: {}", content).into(),
        }
        .into());
    }

    Ok(())
}

fn main() -> Result<()> {
    dotenv().expect(".env file not found");

    match (env::var("API_USER"), env::var("API_TOKEN")) {
        (Err(_), Err(_)) => {
            return Err(Box::from("Missing API_USER and API_TOKEN"));
        }
        (Err(_), Ok(_)) => {
            return Err(Box::from("Missing API_USER"));
        }
        (Ok(_), Err(_)) => {
            return Err(Box::from("Missing API_TOKEN"));
        }
        (Ok(_), Ok(_)) => (),
    }

    let confluence_client = ConfluenceClient::new("jimjim256.atlassian.net");

    let space = match get_space(&confluence_client, "TEAM") {
        Ok(s) => s,
        Err(err) => return Err(err),
    };

    println!("Home page id {}", space.homepage_id);

    let page = Page {
        title: "Hello World".into(),
        content: "<h1>Hello world</h1>".into(),
    };
    match create_page(&confluence_client, space, page) {
        Ok(_) => println!("Page created"),
        Err(err) => return Err(err),
    }

    Ok(())
}
