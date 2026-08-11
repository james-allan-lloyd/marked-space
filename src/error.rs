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

    #[error("Duplicate title '{title}' in [{file}]")]
    DuplicateTitle { title: String, file: String },

    #[error("Missing file for link in [{source_file}] to [{local_links}]")]
    MissingFileLink {
        source_file: String,
        local_links: String,
    },

    #[error("Missing file for attachment link in [{source_file}] to [{attachment_paths}]")]
    MissingAttachmentLink {
        source_file: String,
        attachment_paths: String,
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
            StatusCode::TOO_MANY_REQUESTS => String::from(
                "Rate limited by Confluence. Retries were exhausted: \
                 slow the sync down or allow more retries with $MARKED_SPACE_MAX_RETRIES.",
            ),
            _ => describe_error_body(response),
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

/// Describe the body of a failed response. Confluence answers with JSON for most errors, but
/// gateway and rate limiting responses are often HTML or plain text, so fall back to the raw body.
fn describe_error_body(response: Response) -> String {
    const MAX_BODY_LENGTH: usize = 512;

    let body = match response.text() {
        Ok(body) => body,
        Err(err) => return format!("could not read response body: {}", err),
    };

    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(json) if !json["errors"][0].is_null() => json["errors"][0].to_string(),
        Ok(json) if !json["message"].is_null() => json["message"].to_string(),
        _ => {
            let body = body.trim();
            if body.len() > MAX_BODY_LENGTH {
                format!(
                    "{}...",
                    body.chars().take(MAX_BODY_LENGTH).collect::<String>()
                )
            } else {
                String::from(body)
            }
        }
    }
}

pub type Result<T> = anyhow::Result<T>;

#[cfg(test)]
pub type TestResult = Result<()>;
