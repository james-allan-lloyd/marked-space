use std::fmt;
use std::process::{ExitCode, Termination};

use reqwest::{blocking::Response, StatusCode};

#[derive(Debug)]
pub enum ConfluenceError {
    GenericError(String),
    FailedRequest {
        status: StatusCode,
        body_content: String,
    },
    ParsingError {
        filename: String,
        message: String,
    },
    UnsupportedStorageFormat {
        message: String,
    },
}

impl ConfluenceError {
    pub fn generic_error(message: impl Into<String>) -> ConfluenceError {
        ConfluenceError::GenericError(message.into())
    }

    pub fn failed_request(response: Response) -> ConfluenceError {
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
    }

    pub fn parsing_error(
        filename: impl Into<String>,
        message: impl Into<String>,
    ) -> ConfluenceError {
        ConfluenceError::ParsingError {
            filename: filename.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfluenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfluenceError::GenericError(message) => write!(f, "{}", message.as_str()),
            ConfluenceError::FailedRequest {
                status,
                body_content,
            } => {
                write!(f, "Failed request: {}: {}", status, body_content)
            }
            ConfluenceError::ParsingError { filename, message } => {
                write!(f, "Failed to parse {}: {}", filename, message)
            }
            ConfluenceError::UnsupportedStorageFormat { message } => {
                write!(f, "Unsupported storage format: {}", message)
            }
        }
    }
}

impl From<reqwest::Error> for ConfluenceError {
    fn from(value: reqwest::Error) -> Self {
        ConfluenceError::GenericError(format!("reqwest error: {}", value.to_string()))
    }
}

impl From<std::io::Error> for ConfluenceError {
    fn from(value: std::io::Error) -> Self {
        ConfluenceError::GenericError(format!("io error: {}", value.to_string()))
    }
}

pub type Result<T> = std::result::Result<T, ConfluenceError>;
