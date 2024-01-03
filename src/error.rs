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

    #[error("Failed to parse {filename}: {message}")]
    ParsingError { filename: String, message: String },

    #[error("Unsupported format: {message:?}")]
    UnsupportedStorageFormat { message: String },

    #[error("Duplicate title '{title}' in [{file}]")]
    DuplicateTitle { title: String, file: String },
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

pub type Result<T> = anyhow::Result<T>;

#[cfg(test)]
pub type TestResult = Result<()>;
