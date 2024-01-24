use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use markdown_space::MarkdownSpace;

mod checksum;
mod confluence_client;
mod confluence_page;
mod confluence_paginator;
mod confluence_space;
mod confluence_storage_renderer;
mod error;
mod helpers;
mod link_generator;
mod local_link;
mod markdown_page;
mod markdown_space;
mod moves;
mod parent;
mod responses;
mod sync;
mod sync_operation;
mod template_renderer;

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

    #[arg(long)]
    host: Option<String>,
}

fn main() -> Result<ExitCode> {
    load_dotenv_if_exists();

    let args = Args::parse();

    check_environment_vars()?;

    let dir = PathBuf::from(args.space.clone());
    let markdown_space = MarkdownSpace::from_directory(&dir)?;

    let host = match (args.host, env::var("CONFLUENCE_HOST").ok()) {
        (Some(host), _) => host,
        (_, Some(envvar)) => envvar,
        _ => {
            eprintln!("Couldn't determine host from either --host or $CONFLUENCE_HOST");
            return Ok(ExitCode::FAILURE);
        }
    };
    let confluence_client = ConfluenceClient::new(host.as_str());

    match sync_space(confluence_client, &markdown_space, args.output) {
        Ok(_) => Ok(ExitCode::SUCCESS),
        Err(err) => {
            eprintln!("Error: {:#}", err);
            Ok(ExitCode::FAILURE)
        }
    }
}

fn load_dotenv_if_exists() {
    if let Err(e) = dotenv() {
        match e {
            dotenvy::Error::Io(io_err) => {
                match io_err.kind() {
                    std::io::ErrorKind::NotFound => (), // do nothing
                    _ => eprintln!("Failure loading .env: {}", io_err),
                }
            }
            _ => eprintln!("Failure loading .env: {}", e),
        }
    }
}
