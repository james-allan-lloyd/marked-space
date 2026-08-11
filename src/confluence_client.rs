#![allow(dead_code)]

use anyhow::Context;
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::RequestBuilder;
use reqwest::Method;
use serde_json::{json, Value};
use std::env;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use crate::console::{print_error, print_warning};
use crate::retry::{classify_error, classify_status, retry_after, RetryConfig};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct ConfluenceClient {
    client: reqwest::blocking::Client,
    api_user: String,
    api_token: String,
    pub hostname: String,
    insecure: bool,
    retry: RetryConfig,
}

pub type Result = anyhow::Result<reqwest::blocking::Response>;

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("marked-space/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

impl ConfluenceClient {
    pub fn new(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").unwrap_or_default(),
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            client: http_client(),
            hostname: String::from(hostname),
            insecure: false,
            retry: RetryConfig::from_env(),
        }
    }

    #[cfg(test)]
    pub fn new_insecure(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").unwrap_or_default(),
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            client: http_client(),
            hostname: String::from(hostname),
            insecure: true,
            // Tests shouldn't spend real time waiting between retries.
            retry: RetryConfig {
                max_retries: 3,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(5),
            },
        }
    }

    pub fn with_retry_config(mut self, retry: RetryConfig) -> ConfluenceClient {
        self.retry = retry;
        self
    }

    fn scheme(&self) -> &str {
        if self.insecure {
            "http"
        } else {
            "https"
        }
    }

    fn rest_api(&self, p: &str) -> String {
        format!("{}://{}/wiki/rest/api/{}", self.scheme(), self.hostname, p)
    }

    fn rest_api_v2(&self, p: &str) -> String {
        format!("{}://{}/wiki/api/v2/{}", self.scheme(), self.hostname, p)
    }

    fn graphql_api(&self) -> String {
        format!("{}://{}/cgraphql", self.scheme(), self.hostname)
    }

    /// Start a request with authentication and the headers every endpoint needs.
    fn request(&self, method: Method, url: String) -> RequestBuilder {
        self.request_with_xsrf_token(method, url, "no-check")
    }

    fn request_with_xsrf_token(
        &self,
        method: Method,
        url: String,
        xsrf_token: &str,
    ) -> RequestBuilder {
        self.client
            .request(method, url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", xsrf_token)
    }

    /// Send a request, retrying rate limited (429) and transient failures.
    fn send(&self, builder: RequestBuilder) -> Result {
        if builder.try_clone().is_none() {
            // Streaming bodies can't be replayed, so there's nothing to retry with.
            return Ok(builder.send()?);
        }

        self.send_retrying(|| {
            builder
                .try_clone()
                .context("Request body cannot be replayed")
        })
    }

    /// Send a request built afresh on every attempt, so that requests with a non replayable body
    /// (attachment uploads) can be retried too.
    fn send_retrying<F>(&self, build_request: F) -> Result
    where
        F: Fn() -> anyhow::Result<RequestBuilder>,
    {
        let mut attempt: u32 = 0;
        loop {
            let request = build_request()?.build()?;
            let method = request.method().clone();
            let path = String::from(request.url().path());

            let result = self.client.execute(request);

            let reason = match &result {
                Ok(response) => classify_status(response.status(), &method),
                Err(err) => classify_error(err, &method),
            };

            let Some(reason) = reason else {
                return Ok(result?);
            };

            if attempt >= self.retry.max_retries {
                print_error(&format!(
                    "{} {}: {}. Giving up after {} attempts.",
                    method,
                    path,
                    reason,
                    attempt + 1
                ));
                return Ok(result?);
            }

            let delay = self.retry.delay_for(
                attempt,
                result.as_ref().ok().and_then(|r| retry_after(r.headers())),
            );
            attempt += 1;

            print_warning(&format!(
                "{} {}: {}. Retrying in {:.1}s ({}/{}).",
                method,
                path,
                reason,
                delay.as_secs_f64(),
                attempt,
                self.retry.max_retries
            ));

            sleep(delay);
        }
    }

    pub fn get_space_by_key(&self, space_key: &str) -> Result {
        self.send(
            self.request(Method::GET, self.rest_api_v2("spaces"))
                .query(&[("keys", space_key)]),
        )
    }

    pub fn create_page(&self, body_json: Value) -> Result {
        self.send(
            self.request(Method::POST, self.rest_api_v2("pages"))
                .json(&body_json),
        )
    }

    pub(crate) fn create_folder(&self, body_json: Value) -> Result {
        self.send(
            self.request(Method::POST, self.rest_api_v2("folders"))
                .json(&body_json),
        )
    }

    pub fn get(&self, url: &reqwest::Url) -> Result {
        self.send(self.request(Method::GET, url.to_string()))
    }

    pub fn get_all_pages_in_space(&self, space_id: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api_v2(&format!("spaces/{}/pages", space_id)),
        ))
    }

    pub fn get_all_pages_from_homepage(&self, homepage_id: &str) -> Result {
        self.send(
            self.request(
                Method::GET,
                self.rest_api_v2(&format!("pages/{}/descendants", homepage_id)),
            )
            .query(&[("limit", "1")]),
        )
    }

    pub(crate) fn get_folder_descendants(&self, page_id: String) -> Result {
        self.send(
            self.request(
                Method::GET,
                self.rest_api_v2(&format!("folders/{}/descendants", page_id)),
            )
            .query(&[("depth", "1")]),
        )
    }

    pub(crate) fn get_page_descendants(&self, page_id: String) -> Result {
        self.send(
            self.request(
                Method::GET,
                self.rest_api_v2(&format!("pages/{}/descendants", page_id)),
            )
            .query(&[("depth", "1")]),
        )
    }

    pub fn update_page(&self, page_id: &String, payload: Value) -> Result {
        self.send(
            self.request(Method::PUT, self.rest_api_v2(&format!("pages/{}", page_id)))
                .json(&payload),
        )
    }

    pub fn create_or_update_attachment(
        &self,
        content_id: &str,
        file: &Path,
        file_name: &str,
        hash: &str,
    ) -> Result {
        let url = self.rest_api(&format!("content/{}/child/attachment", content_id));

        // The form is rebuilt on every attempt because the file is streamed, and a streamed body
        // can only be sent once.
        self.send_retrying(|| {
            let file_part = Part::file(file)
                .with_context(|| format!("Opening {}", file.display()))?
                .file_name(String::from(file_name));
            let form = Form::new()
                .text("minorEdit", "true")
                .text("comment", format!("hash:{}", hash))
                .part("file", file_part);

            Ok(self
                .request_with_xsrf_token(Method::PUT, url.clone(), "nocheck")
                .multipart(form))
        })
    }

    pub fn get_attachments(&self, page_id: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api_v2(&format!("pages/{}/attachments", page_id)),
        ))
    }

    pub(crate) fn remove_attachment(&self, id: &str) -> Result {
        self.send(self.request(
            Method::DELETE,
            self.rest_api_v2(&format!("attachments/{}", id)),
        ))
    }

    pub(crate) fn get_page_labels(&self, page_id: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api(&format!("content/{}/label", page_id)),
        ))
    }

    pub(crate) fn set_page_labels(&self, page_id: &str, body: Vec<Value>) -> Result {
        self.send(
            self.request(
                Method::POST,
                self.rest_api(&format!("content/{}/label", page_id)),
            )
            .json(&body),
        )
    }

    pub(crate) fn remove_label(&self, page_id: &str, label: &crate::responses::Label) -> Result {
        self.send(
            self.request(
                Method::DELETE,
                self.rest_api(&format!("content/{}/label", page_id)),
            )
            .query(&[("name", label.name.clone())]),
        )
    }

    pub(crate) fn get_properties(&self, page_id: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api_v2(&format!("pages/{}/properties", page_id)),
        ))
    }

    pub(crate) fn create_property(&self, page_id: &str, value: Value) -> Result {
        self.send(
            self.request(
                Method::POST,
                self.rest_api_v2(&format!("pages/{}/properties", page_id)),
            )
            .json(&value),
        )
    }

    pub(crate) fn set_property(&self, page_id: &str, property_id: &str, value: Value) -> Result {
        self.send(
            self.request(
                Method::PUT,
                self.rest_api_v2(&format!("pages/{}/properties/{}", page_id, property_id)),
            )
            .json(&value),
        )
    }

    pub(crate) fn delete_property(&self, page_id: &str, property_id: &str) -> Result {
        self.send(self.request(
            Method::DELETE,
            self.rest_api_v2(&format!("pages/{}/properties/{}", page_id, property_id)),
        ))
    }

    pub(crate) fn search_users(&self, public_name: &str) -> Result {
        self.send(
            self.request(Method::GET, self.rest_api("search/user"))
                .query(&[("cql", format!("user.fullname~\"{}\"", public_name))]),
        )
    }

    pub(crate) fn archive_page(&self, id: &str, note: &str) -> Result {
        self.send(
            self.request(Method::POST, self.graphql_api())
                .query(&[("q", "ArchivePagesMutation")])
                .json(&json!({
                    "operationName": "ArchivePagesMutation",
                    "variables": {
                        "input": [
                            { "pageID": id, "archiveNote": note, "descendantsNoteApplicationOption": "NONE", "areChildrenIncluded": false}
                        ]
                    },
                    "query": "mutation ArchivePagesMutation($input: [BulkArchivePagesInput]!) {\narchivePages(input: $input) {\n    taskId\n    status\n    __typename\n  }\n}\n"
                })),
        )
    }

    pub(crate) fn unarchive_page(&self, id: &str) -> Result {
        self.send(
            self.request(Method::POST, self.graphql_api())
                .query(&[("q", "ArchivePagesMutation")])
                .json(&json!({
                    "operationName": "UnarchivePagesMutation",
                    "variables": {
                        "pageIDs": [ id ],
                        "includeChildren": false
                    },
                    "query": "mutation UnarchivePagesMutation($pageIDs: [Long!]!, $includeChildren: [Boolean!]!, $parentPageId: Long) {\n  bulkUnarchivePages(\n    pageIDs: $pageIDs\n    includeChildren: $includeChildren\n    parentPageId: $parentPageId\n  ) {\n    taskId\n    status\n    __typename\n  }\n}\n"
                })),
        )
    }

    pub(crate) fn move_page(&self, page_id: &str, parent_id: &str) -> Result {
        self.send(
            self.request(Method::POST, self.graphql_api())
                .query(&[("q", "useMovePageHandlerMovePageAppendMutation")])
                .json(&json!({
                    "operationName": "useMovePageHandlerMovePageAppendMutation",
                    "variables": {
                        "pageId": page_id,
                        "parentId": parent_id,
                    },
                    "query": "mutation useMovePageHandlerMovePageAppendMutation($pageId: ID!, $parentId: ID!) {\n  movePageAppend(input: {pageId: $pageId, parentId: $parentId}) {\n    page {\n      id\n      links {\n        webui\n        editui\n        __typename\n      }\n      __typename\n    }\n    __typename\n  }\n}\n"
                })),
        )
    }

    pub(crate) fn set_restrictions(&self, id: &str, body: Value) -> Result {
        self.send(
            self.request(
                Method::PUT,
                self.rest_api(&format!("content/{}/restriction", id)),
            )
            .json(&body),
        )
    }

    pub(crate) fn current_user(&self) -> Result {
        self.send(self.request(Method::GET, self.rest_api("user/current")))
    }

    pub(crate) fn delete_restrictions(&self, id: &str) -> Result {
        self.send(self.request(
            Method::DELETE,
            self.rest_api(&format!("content/{}/restriction", id)),
        ))
    }

    pub(crate) fn get_restrictions_by_operation(&self, id: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api(&format!("content/{}/restriction/byOperation", id)),
        ))
    }

    pub(crate) fn move_page_relative(
        &self,
        page_id: &str,
        position: &str,
        target_id: &str,
    ) -> Result {
        self.send(self.request(
            Method::PUT,
            self.rest_api(&format!(
                "content/{}/move/{}/{}",
                page_id, position, target_id
            )),
        ))
    }

    pub(crate) fn get_space_suggested_content_states(&self, space_key: &str) -> Result {
        self.send(self.request(
            Method::GET,
            self.rest_api(&format!("space/{}/state", space_key)),
        ))
    }

    pub(crate) fn set_content_state(&self, id: &str, status: &str, body: Value) -> Result {
        self.send(
            self.request(Method::PUT, self.rest_api(&format!("content/{}/state", id)))
                .query(&[("status", status)])
                .json(&body),
        )
    }

    pub(crate) fn get_content_state(&self, id: &str) -> Result {
        self.send(self.request(Method::GET, self.rest_api(&format!("content/{}/state", id))))
    }

    pub(crate) fn remove_content_state(&self, id: &str, status: &str) -> Result {
        self.send(
            self.request(
                Method::DELETE,
                self.rest_api(&format!("content/{}/state", id)),
            )
            .query(&[("status", status)]),
        )
    }
}

#[cfg(test)]
mod test {
    use assert_fs::fixture::{FileWriteStr, PathChild};
    use mockito::Matcher;
    use serde_json::json;

    use crate::error::TestResult;

    use super::*;

    fn page_body() -> String {
        json!({ "id": "1", "title": "A Page" }).to_string()
    }

    #[test]
    fn it_retries_rate_limited_requests() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        let rate_limited = server
            .mock("GET", "/wiki/api/v2/spaces")
            .match_query(Matcher::Any)
            .with_status(429)
            .with_header("retry-after", "0")
            .with_body("Rate limit exceeded")
            .expect(2)
            .create();

        let ok = server
            .mock("GET", "/wiki/api/v2/spaces")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(json!({ "results": [] }).to_string())
            .create();

        let response = client.get_space_by_key("TEST")?;

        assert_eq!(response.status(), 200);
        rate_limited.assert();
        ok.assert();

        Ok(())
    }

    #[test]
    fn it_gives_up_after_the_configured_number_of_retries() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port()).with_retry_config(
            RetryConfig {
                max_retries: 2,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(5),
            },
        );

        // The initial attempt plus two retries.
        let rate_limited = server
            .mock("GET", "/wiki/api/v2/spaces")
            .match_query(Matcher::Any)
            .with_status(429)
            .expect(3)
            .create();

        let response = client.get_space_by_key("TEST")?;

        assert_eq!(response.status(), 429);
        rate_limited.assert();

        Ok(())
    }

    #[test]
    fn it_retries_rate_limited_writes() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        let rate_limited = server
            .mock("POST", "/wiki/api/v2/pages")
            .with_status(429)
            .expect(1)
            .create();

        let created = server
            .mock("POST", "/wiki/api/v2/pages")
            .with_status(200)
            .with_body(page_body())
            .create();

        let response = client.create_page(json!({"title": "A Page"}))?;

        assert_eq!(response.status(), 200);
        rate_limited.assert();
        created.assert();

        Ok(())
    }

    #[test]
    fn it_retries_rate_limited_attachment_uploads() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        let temp = assert_fs::TempDir::new()?;
        let attachment = temp.child("image.png");
        attachment.write_str("not really a png")?;

        let rate_limited = server
            .mock("PUT", "/wiki/rest/api/content/1/child/attachment")
            .with_status(429)
            .expect(1)
            .create();

        let uploaded = server
            .mock("PUT", "/wiki/rest/api/content/1/child/attachment")
            .with_status(200)
            .with_body(json!({ "results": [] }).to_string())
            .create();

        let response =
            client.create_or_update_attachment("1", attachment.path(), "image.png", "hash")?;

        assert_eq!(response.status(), 200);
        rate_limited.assert();
        uploaded.assert();

        Ok(())
    }

    #[test]
    fn it_retries_transient_server_errors_on_reads() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        let unavailable = server
            .mock("GET", "/wiki/api/v2/pages/1/properties")
            .with_status(503)
            .expect(1)
            .create();

        let ok = server
            .mock("GET", "/wiki/api/v2/pages/1/properties")
            .with_status(200)
            .with_body(json!({ "results": [] }).to_string())
            .create();

        let response = client.get_properties("1")?;

        assert_eq!(response.status(), 200);
        unavailable.assert();
        ok.assert();

        Ok(())
    }

    #[test]
    fn it_does_not_retry_server_errors_on_creates() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        // Repeating a failed create could duplicate the page, so it must be attempted once only.
        let failed = server
            .mock("POST", "/wiki/api/v2/pages")
            .with_status(500)
            .expect(1)
            .create();

        let response = client.create_page(json!({"title": "A Page"}))?;

        assert_eq!(response.status(), 500);
        failed.assert();

        Ok(())
    }

    #[test]
    fn it_does_not_retry_client_errors() -> TestResult {
        let mut server = mockito::Server::new();
        let client = ConfluenceClient::new_insecure(&server.host_with_port());

        let unauthorized = server
            .mock("GET", "/wiki/api/v2/spaces")
            .match_query(Matcher::Any)
            .with_status(401)
            .expect(1)
            .create();

        let response = client.get_space_by_key("TEST")?;

        assert_eq!(response.status(), 401);
        unauthorized.assert();

        Ok(())
    }
}
