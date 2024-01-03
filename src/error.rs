use std::path::Display;

use reqwest::{blocking::Response, StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfluenceError {
    #[error("{0}")]
    GenericError(String),

    #[error("Failed request {status}: {body_content}")]
    FailedRequest {
        status: StatusCode,
        body_content: String,
    },

    #[error("Failed to parse {filename}: {errors}")]
    ParsingError { filename: String, errors: String },

    #[error("Unsupported format: {message:?}")]
    UnsupportedStorageFormat { message: String },

    #[error("Duplicate title '{title}' in [{file}]")]
    DuplicateTitle { title: String, file: String },

    #[error("Missing file for link in [{source_file}] to [{local_link}]")]
    MissingFileLink {
        source_file: String,
        local_link: String,
    },
}

impl ConfluenceError {
    pub fn generic_error(message: impl Into<String>) -> anyhow::Error {
        ConfluenceError::GenericError(message.into()).into()
    }

    pub fn failed_request(response: Response) -> anyhow::Error {
        let status = response.status();
        let body_content = match status {
            StatusCode::UNAUTHORIZED => {
                String::from("Unauthorized. Check your API_USER/API_TOKEN and try again.")
            }
            _ => {
                let json: serde_json::Value = match response.json() {
                    Ok(j) => j,
                    Err(_) => todo!(),
                };
                json["errors"][0]["title"].to_string()
            }
        };
        ConfluenceError::FailedRequest {
            status,
            body_content,
        }
        .into()
    }

    pub fn parsing_errors(filename: impl Into<String>, errors: Vec<String>) -> anyhow::Error {
        let errors = errors.join(", ");
        ConfluenceError::ParsingError {
            filename: filename.into(),
            errors,
        }
        .into()
    }
}

pub type Result<T> = anyhow::Result<T>;

#[cfg(test)]
pub type TestResult = Result<()>;
