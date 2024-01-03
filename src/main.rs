use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use markdown_space::MarkdownSpace;

mod checksum;
mod confluence_client;
mod error;
mod html;
mod markdown_page;
mod markdown_space;
mod parent;
mod responses;
mod sync;

use crate::error::{ConfluenceError, Result};
use crate::sync::sync_space;

fn check_environment_vars() -> Result<()> {
    match (env::var("API_USER"), env::var("API_TOKEN")) {
        (Err(_), Err(_)) => Err(ConfluenceError::generic_error(
            "Missing API_USER and API_TOKEN",
        )),
        (Err(_), Ok(_)) => Err(ConfluenceError::generic_error("Missing API_USER")),
        (Ok(_), Err(_)) => Err(ConfluenceError::generic_error("Missing API_TOKEN")),
        (Ok(_), Ok(_)) => Ok(()),
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the space to update
    #[arg(short, long)]
    space: String,

    /// Write intermediate output to this directory
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> Result<ExitCode> {
    dotenv().expect(".env file not found");

    let args = Args::parse();

    check_environment_vars()?;

    let dir = PathBuf::from(args.space.clone());
    let markdown_space = MarkdownSpace::from_directory(&dir)?;

    println!(
        "Syncing {} pages from space {}",
        markdown_space.markdown_pages.len(),
        args.space
    );

    let confluence_client = ConfluenceClient::new("jimjim256.atlassian.net");

    match sync_space(confluence_client, &markdown_space, args.output) {
        Ok(_) => Ok(ExitCode::SUCCESS),
        Err(err) => {
            println!("Error: {}", err);
            Ok(ExitCode::FAILURE)
        }
    }
}
