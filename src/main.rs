use std::env;
use std::fmt;

use comrak::markdown_to_html;
use comrak::Options;
use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use serde_json::json;

mod confluence_client;

#[derive(Debug, Clone)]
struct ConfluenceError {
    message: String,
}

impl ConfluenceError {
    fn new(message: impl Into<String>) -> ConfluenceError {
        ConfluenceError {
            message: message.into(),
        }
    }
}

impl std::error::Error for ConfluenceError {}

impl fmt::Display for ConfluenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error {}", self.message.as_str())
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn get_space(confluence_client: &ConfluenceClient, space_id: &str) -> Result<responses::Space> {
    let resp = match confluence_client.get_space_by_key(space_id) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(ConfluenceError::new("Failed to get space id").into());
        }
    };

    if !resp.status().is_success() {
        return Err(ConfluenceError::new(format!(
            "Failed: {}",
            resp.text().unwrap_or("no text content".into())
        ))
        .into());
    }
    let json = resp.json::<serde_json::Value>()?;

    match serde_json::from_value::<responses::Space>(json["results"][0].clone()) {
        Ok(parsed_space) => return Ok(parsed_space),
        Err(_) => return Err(ConfluenceError::new("Failed to parse response.").into()),
    };
}

struct Page {
    title: String,
    content: String,
}

mod responses {
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    pub enum BodyBulk {
        #[serde(rename = "storage")]
        Storage {
            representation: String,
            value: String,
        },
        #[serde(rename = "atlas_doc_format")]
        AtlasDocFormat {
            representation: String,
            value: String,
        },
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Version {
        pub message: String,
        pub number: i32,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct PageBulk {
        pub id: String,
        pub title: String,
        pub version: Version,
        pub body: BodyBulk,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct MultiEntityResult {
        pub results: Vec<PageBulk>,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Space {
        pub id: String,
        pub _key: String,
        pub _name: String,
        pub homepage_id: String,
    }
}

fn sync_page(
    confluence_client: &ConfluenceClient,
    space: responses::Space,
    page: Page,
) -> Result<()> {
    let mut payload = json!({
        "spaceId": space.id,
        "status": "current",
        "title": page.title,
        "parentId": space.homepage_id,
        "body": {
            "representation": "storage",
            "value": page.content
        }
    });

    let existing_page = match confluence_client.get_page_by_title(&space.id, page.title.as_str()) {
        Ok(resp) => resp,
        Err(_) => todo!(),
    };

    let json: responses::MultiEntityResult =
        match serde_json::from_str(existing_page.text().unwrap_or_default().as_str()) {
            Ok(j) => j,
            Err(_) => todo!(),
        };

    if json.results.is_empty() {
        println!("Page doesn't exist, creating");
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
        if !status.is_success() {
            return Err(ConfluenceError {
                message: format!("Failed to create page: {}", content).into(),
            }
            .into());
        }
    } else {
        println!("Updating");

        // println!("body {:#?}", json.results[0].body);
        let existing_content = match &json.results[0].body {
            responses::BodyBulk::Storage {
                representation: _,
                value,
            } => value,
            responses::BodyBulk::AtlasDocFormat {
                representation: _,
                value: _,
            } => todo!(),
        };

        if *existing_content == page.content {
            println!("Already up to date");
            return Ok(());
        }
        let id = json.results[0].id.clone();
        payload["id"] = id.clone().into();
        payload["version"] = json!({
            "message": "updated automatically",
            "number": json.results[0].version.number + 1
        });
        let resp = match confluence_client.update_page(id, payload) {
            Ok(r) => r,
            Err(_) => {
                return Err(ConfluenceError {
                    message: "Failed to update page".into(),
                }
                .into())
            }
        };

        if !resp.status().is_success() {
            return Err(ConfluenceError {
                message: format!("Failed to update page: {:?}", resp.text()).into(),
            }
            .into());
        }
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

    let page = Page {
        title: "Hello World".into(),
        content: markdown_to_html(
            "Hello, **世界**!\n\n## Heading 2\n\nSome subsection.",
            &Options::default(),
        ),
    };

    match sync_page(&confluence_client, space, page) {
        Ok(_) => println!("Page synced"),
        Err(err) => return Err(err),
    }

    Ok(())
}
