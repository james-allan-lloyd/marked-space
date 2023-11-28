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
}

impl ConfluenceError {
    pub fn new(message: impl Into<String>) -> ConfluenceError {
        ConfluenceError::GenericError(message.into())
    }

    pub fn failed_request(response: Response) -> ConfluenceError {
        let status = response.status();
        let body_content = match status {
            StatusCode::UNAUTHORIZED => {
                String::from("Unauthorized. Check your API_USER/API_TOKEN and try again.")
            }
            _ => response.text().unwrap_or("No content".into()),
        };
        ConfluenceError::FailedRequest {
            status,
            body_content,
        }
    }
}

impl std::error::Error for ConfluenceError {}

impl Termination for ConfluenceError {
    fn report(self) -> std::process::ExitCode {
        println!("** Error: {}", self);
        return ExitCode::FAILURE;
    }
}

impl fmt::Display for ConfluenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfluenceError::GenericError(message) => write!(f, "error: {}", message.as_str()),
            ConfluenceError::FailedRequest {
                status,
                body_content,
            } => {
                write!(f, "failed request: {}: {}", status, body_content)
            }
        }
    }
}

impl From<reqwest::Error> for ConfluenceError {
    fn from(value: reqwest::Error) -> Self {
        ConfluenceError::GenericError(format!("reqwest error: {:#?}", value))
    }
}

pub type Result<T> = std::result::Result<T, ConfluenceError>;
