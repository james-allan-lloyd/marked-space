use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use confluence_client::ConfluenceClient;
use dotenvy::dotenv;
use markdown_space::MarkdownSpace;

mod archive;
mod attachment;
mod checksum;
mod confluence_client;
mod confluence_page;
mod confluence_paginator;
mod confluence_space;
mod confluence_storage_renderer;
mod console;
mod error;
mod frontmatter;
mod helpers;
mod imports;
mod link_generator;
mod local_link;
mod markdown_page;
mod markdown_space;
mod mentions;
mod page_emojis;
mod parent;
mod responses;
mod restrictions;
mod sync;
mod sync_operation;
mod template_renderer;
#[cfg(test)]
mod test_helpers;

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

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the space to update
    #[arg(short, long)]
    space: String,

    /// Write intermediate output to this directory
    #[arg(short, long)]
    output: Option<String>,

    /// The host to connect to. Can also be specified in $CONFLUENCE_HOST
    #[arg(long)]
    host: Option<String>,

    /// Set the user identified by the token to the sole editor of pages. Default is to make the
    /// space editable to anyone who has access to the space.
    #[arg(long)]
    single_editor: bool,
}

fn main() -> Result<ExitCode> {
    load_dotenv_if_exists();

    let args = Args::parse();

    check_environment_vars()?;

    let dir = PathBuf::from(args.space.clone());
    let markdown_space = MarkdownSpace::from_directory(&dir)?;

    let host = match (args.host.clone(), env::var("CONFLUENCE_HOST").ok()) {
        (Some(host), _) => host,
        (_, Some(envvar)) => envvar,
        _ => {
            eprintln!("Couldn't determine host from either --host or $CONFLUENCE_HOST");
            return Ok(ExitCode::FAILURE);
        }
    };
    let confluence_client = ConfluenceClient::new(host.as_str());

    match sync_space(confluence_client, &markdown_space, args) {
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
