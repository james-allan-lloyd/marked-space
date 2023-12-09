use std::fmt;

use reqwest::{blocking::Response, StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfluenceError {
    #[error("{0}")]
    GenericError(String),

    #[error("Failed request {status:?}: {body_content:?}")]
    FailedRequest {
        status: StatusCode,
        body_content: String,
    },

    #[error("Failed to parse {filename}: {message}")]
    ParsingError { filename: String, message: String },

    #[error("Unsupported format: {message:?}")]
    UnsupportedStorageFormat { message: String },
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

    pub fn parsing_error(filename: impl Into<String>, message: impl Into<String>) -> anyhow::Error {
        ConfluenceError::ParsingError {
            filename: filename.into(),
            message: message.into(),
        }
        .into()
    }
}

// impl fmt::Display for ConfluenceError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             ConfluenceError::GenericError(message) => write!(f, "{}", message.as_str()),
//             ConfluenceError::FailedRequest {
//                 status,
//                 body_content,
//             } => {
//                 write!(f, "Failed request: {}: {}", status, body_content)
//             }
//             ConfluenceError::ParsingError { filename, message } => {
//                 write!(f, "Failed to parse {}: {}", filename, message)
//             }
//             ConfluenceError::UnsupportedStorageFormat { message } => {
//                 write!(f, "Unsupported storage format: {}", message)
//             }
//         }
//     }
// }

impl From<reqwest::Error> for ConfluenceError {
    fn from(value: reqwest::Error) -> Self {
        ConfluenceError::GenericError(format!("reqwest error: {}", value))
    }
}

impl From<std::io::Error> for ConfluenceError {
    fn from(value: std::io::Error) -> Self {
        ConfluenceError::GenericError(format!("io error: {}", value))
    }
}

pub type Result<T> = anyhow::Result<T>;
