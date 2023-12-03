use std::env;
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use comrak::nodes::AstNode;
use comrak::Arena;
use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use markdown_page::MarkdownPage;
use markdown_space::MarkdownSpace;
use serde_json::json;

mod confluence_client;
mod error;
mod markdown_page;
mod markdown_space;
mod responses;

use crate::error::{ConfluenceError, Result};

fn get_space(confluence_client: &ConfluenceClient, space_id: &str) -> Result<responses::Space> {
    let resp = match confluence_client.get_space_by_key(space_id) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(ConfluenceError::new("Failed to get space id").into());
        }
    };

    if !resp.status().is_success() {
        return Err(ConfluenceError::failed_request(resp));
    }

    let json = resp.json::<serde_json::Value>()?;

    if json["results"].as_array().unwrap().is_empty() {
        return Err(ConfluenceError::new(format!("No such space: {}", space_id)));
    }

    match serde_json::from_value::<responses::Space>(json["results"][0].clone()) {
        Ok(parsed_space) => return Ok(parsed_space),
        Err(_) => return Err(ConfluenceError::new("Failed to parse response.").into()),
    };
}

struct Page {
    title: String,
    content: String,
    source: String,
}

fn parse_page(markdown_page: &Path) -> Result<Page> {
    // The returned nodes are created in the supplied Arena, and are bound by its lifetime.

    let arena = Arena::<AstNode>::new();

    let markdown_page = MarkdownPage::parse(markdown_page, &arena)?;

    let content = markdown_page.to_html_string()?.clone();

    Ok(Page {
        title: markdown_page.title.clone(),
        content,
        source: markdown_page.source.clone(),
    })
}

fn sync_page(
    confluence_client: &ConfluenceClient,
    space: &responses::Space,
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
            Err(_) => return Err(ConfluenceError::new("Failed to create page").into()),
        };

        let status = resp.status();
        let content = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(ConfluenceError::new(format!("Failed to create page: {}", content)).into());
        }
    } else {
        let id = json.results[0].id.clone();
        println!("Updating \"{}\" ({}) from {}", page.title, id, page.source);

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
        payload["id"] = id.clone().into();
        payload["version"] = json!({
            "message": "updated automatically",
            "number": json.results[0].version.number + 1
        });

        let resp = match confluence_client.update_page(id, payload) {
            Ok(r) => r,
            Err(_) => return Err(ConfluenceError::new("Failed to update page").into()),
        };

        if !resp.status().is_success() {
            return Err(
                ConfluenceError::new(format!("Failed to update page: {:?}", resp.text())).into(),
            );
        }
    }

    println!("Page synced");
    Ok(())
}

fn sync_space(confluence_client: ConfluenceClient, markdown_space: &MarkdownSpace) -> Result<()> {
    let space_key = markdown_space.key.clone();
    println!("Updating space {}...", space_key);
    let space = get_space(&confluence_client, space_key.as_str())?;

    for markdown_page in markdown_space.markdown_pages.iter() {
        let page = parse_page(markdown_page)?;

        sync_page(&confluence_client, &space, page)?;
    }

    Ok(())
}

fn check_environment_vars() -> Result<()> {
    match (env::var("API_USER"), env::var("API_TOKEN")) {
        (Err(_), Err(_)) => {
            return Err(ConfluenceError::new("Missing API_USER and API_TOKEN"));
        }
        (Err(_), Ok(_)) => {
            return Err(ConfluenceError::new("Missing API_USER"));
        }
        (Ok(_), Err(_)) => {
            return Err(ConfluenceError::new("Missing API_TOKEN"));
        }
        (Ok(_), Ok(_)) => Ok(()),
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the space to update
    #[arg(short, long)]
    space: String,
}

fn main() -> Result<()> {
    dotenv().expect(".env file not found");

    let args = Args::parse();

    check_environment_vars()?;

    let dir = PathBuf::from(args.space.clone());
    let markdown_space = MarkdownSpace::from_directory(&dir)?;

    println!(
        "Syncing {} from space {}",
        markdown_space.markdown_pages.len(),
        args.space
    );

    let confluence_client = ConfluenceClient::new("jimjim256.atlassian.net");

    sync_space(confluence_client, &markdown_space)?;

    Ok(())
}
