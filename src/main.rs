use std::env;
use std::path::PathBuf;

use clap::Parser;

use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use markdown_space::MarkdownSpace;

mod confluence_client;
mod error;
mod markdown_page;
mod markdown_space;
mod responses;
mod sync;

use crate::error::{ConfluenceError, Result};
use crate::sync::sync_space;

fn check_environment_vars() -> Result<()> {
    match (env::var("API_USER"), env::var("API_TOKEN")) {
        (Err(_), Err(_)) => {
            return Err(ConfluenceError::generic_error(
                "Missing API_USER and API_TOKEN",
            ));
        }
        (Err(_), Ok(_)) => {
            return Err(ConfluenceError::generic_error("Missing API_USER"));
        }
        (Ok(_), Err(_)) => {
            return Err(ConfluenceError::generic_error("Missing API_TOKEN"));
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
