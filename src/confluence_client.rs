#![allow(dead_code)]

use reqwest::blocking::multipart::{Form, Part};
use serde_json::{json, Value};
use std::env;

#[derive(Clone)]
pub struct ConfluenceClient {
    client: reqwest::blocking::Client,
    api_user: String,
    api_token: String,
    pub hostname: String,
    insecure: bool,
}

pub type Result = anyhow::Result<reqwest::blocking::Response, reqwest::Error>;

impl ConfluenceClient {
    pub fn new(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").unwrap_or_default(),
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            client: reqwest::blocking::Client::new(),
            hostname: String::from(hostname),
            insecure: false,
        }
    }

    #[cfg(test)]
    pub fn new_insecure(hostname: &str) -> ConfluenceClient {
        ConfluenceClient {
            api_user: env::var("API_USER").unwrap_or_default(),
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            client: reqwest::blocking::Client::new(),
            hostname: String::from(hostname),
            insecure: true,
        }
    }

    fn rest_api(&self, p: &str) -> String {
        format!(
            "{}://{}/wiki/rest/api/{}",
            if self.insecure { "http" } else { "https" },
            self.hostname,
            p
        )
    }

    fn rest_api_v2(&self, p: &str) -> String {
        format!(
            "{}://{}/wiki/api/v2/{}",
            if self.insecure { "http" } else { "https" },
            self.hostname,
            p
        )
    }

    fn graphql_api(&self) -> String {
        format!(
            "{}://{}/cgraphql",
            if self.insecure { "http" } else { "https" },
            self.hostname
        )
    }

    pub fn get_space_by_key(&self, space_key: &str) -> Result {
        let url = format!("https://{}/wiki/api/v2/spaces", self.hostname);
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .query(&[("keys", space_key)])
            .send()
    }

    pub fn create_page(&self, body_json: Value) -> Result {
        let url = format!("https://{}/wiki/api/v2/pages", self.hostname);
        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .json(&body_json)
            .send()
    }

    pub(crate) fn create_folder(&self, body_json: Value) -> Result {
        let url = self.rest_api_v2("folders");
        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .json(&body_json)
            .send()
    }

    pub fn get(&self, url: &reqwest::Url) -> Result {
        self.client
            .get(url.clone())
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .send()
    }

    pub fn get_all_pages_in_space(&self, space_id: &str) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/spaces/{}/pages",
            self.hostname, space_id
        );

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .send()
    }

    pub fn get_all_pages_from_homepage(&self, homepage_id: &str) -> Result {
        let url = self.rest_api_v2(&format!("pages/{}/descendants", homepage_id));

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .query(&[("limit", "1")])
            .header("Accept", "application/json")
            .send()
    }

    pub(crate) fn get_folder_descendants(&self, page_id: String) -> Result {
        let url = self.rest_api_v2(&format!("folders/{}/descendants", page_id));

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .query(&[("depth", "1")])
            .header("Accept", "application/json")
            .send()
    }

    pub(crate) fn get_page_descendants(&self, page_id: String) -> Result {
        let url = self.rest_api_v2(&format!("pages/{}/descendants", page_id));

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .query(&[("depth", "1")])
            .header("Accept", "application/json")
            .send()
    }

    pub fn update_page(&self, page_id: &String, payload: Value) -> Result {
        let url = format!("https://{}/wiki/api/v2/pages/{}", self.hostname, page_id);
        // let url = format!("https://{}/wiki/api/content/{}", self.hostname, page_id);
        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .json(&payload)
            .send()
    }

    pub fn create_or_update_attachment(
        &self,
        content_id: &str,
        file_part: Part,
        hash: &String,
    ) -> Result {
        let url = format!(
            "https://{}/wiki/rest/api/content/{}/child/attachment",
            self.hostname, content_id
        );
        let form = Form::new()
            .text("minorEdit", "true")
            .text("comment", format!("hash:{}", hash))
            .part("file", file_part);

        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "nocheck")
            .multipart(form)
            .send()
    }

    pub fn get_attachments(&self, page_id: &str) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}/attachments",
            self.hostname, page_id
        );

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .send()
    }

    pub(crate) fn remove_attachment(&self, id: &str) -> Result {
        let url = format!("https://{}/wiki/api/v2/attachments/{}", self.hostname, id);

        self.client
            .delete(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .send()
    }

    pub(crate) fn get_page_labels(&self, page_id: &str) -> Result {
        let url = format!(
            "https://{}/wiki/rest/api/content/{}/label",
            self.hostname, page_id
        );

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn set_page_labels(&self, page_id: &str, body: Vec<Value>) -> Result {
        let url = format!(
            "https://{}/wiki/rest/api/content/{}/label",
            self.hostname, page_id
        );

        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .json(&body)
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn remove_label(&self, page_id: &str, label: &crate::responses::Label) -> Result {
        let url = format!(
            "https://{}/wiki/rest/api/content/{}/label",
            self.hostname, page_id
        );

        self.client
            .delete(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .query(&[("name", label.name.clone())])
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn get_properties(&self, page_id: &str) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}/properties",
            self.hostname, page_id
        );

        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn create_property(&self, page_id: &str, value: Value) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}/properties",
            self.hostname, page_id
        );

        self.client
            .post(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&value)
            .send()
    }

    pub(crate) fn set_property(&self, page_id: &str, property_id: &str, value: Value) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}/properties/{}",
            self.hostname, page_id, property_id
        );

        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&value)
            .send()
    }

    pub(crate) fn delete_property(&self, page_id: &str, property_id: &str) -> Result {
        let url = format!(
            "https://{}/wiki/api/v2/pages/{}/properties/{}",
            self.hostname, page_id, property_id
        );

        self.client
            .delete(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn search_users(&self, public_name: &str) -> Result {
        let url = self.rest_api("search/user");
        self.client
            .get(url)
            .query(&[("cql", format!("user.fullname~\"{}\"", public_name))])
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn archive_page(&self, id: &str, note: &str) -> Result {
        let url = self.graphql_api();
        self.client
            .post(url)
            .query(&[("q", "ArchivePagesMutation")])
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&json!({
                "operationName": "ArchivePagesMutation",
                "variables": {
                    "input": [
                        { "pageID": id, "archiveNote": note, "descendantsNoteApplicationOption": "NONE", "areChildrenIncluded": false}
                    ]
                },
                "query": "mutation ArchivePagesMutation($input: [BulkArchivePagesInput]!) {\narchivePages(input: $input) {\n    taskId\n    status\n    __typename\n  }\n}\n"
            }))
            .send()
    }

    pub(crate) fn unarchive_page(&self, id: &str) -> Result {
        let url = self.graphql_api();
        self.client
            .post(url)
            .query(&[("q", "ArchivePagesMutation")])
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&json!({
                "operationName": "UnarchivePagesMutation",
                "variables": {
                    "pageIDs": [ id ],
                    "includeChildren": false
                },
                "query": "mutation UnarchivePagesMutation($pageIDs: [Long!]!, $includeChildren: [Boolean!]!, $parentPageId: Long) {\n  bulkUnarchivePages(\n    pageIDs: $pageIDs\n    includeChildren: $includeChildren\n    parentPageId: $parentPageId\n  ) {\n    taskId\n    status\n    __typename\n  }\n}\n"
            }))
            .send()
    }

    pub(crate) fn move_page(&self, page_id: &str, parent_id: &str) -> Result {
        let url = self.graphql_api();
        self.client
            .post(url)
            .query(&[("q", "useMovePageHandlerMovePageAppendMutation")])
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&json!({
                    "operationName": "useMovePageHandlerMovePageAppendMutation",
                    "variables": {
                        "pageId": page_id,
                        "parentId": parent_id,
                    },
                    "query": "mutation useMovePageHandlerMovePageAppendMutation($pageId: ID!, $parentId: ID!) {\n  movePageAppend(input: {pageId: $pageId, parentId: $parentId}) {\n    page {\n      id\n      links {\n        webui\n        editui\n        __typename\n      }\n      __typename\n    }\n    __typename\n  }\n}\n"
                }))
            .send()
    }

    pub(crate) fn set_restrictions(&self, id: &str, body: Value) -> Result {
        let url = self.rest_api(&format!("content/{}/restriction", id));
        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .json(&body)
            .send()
    }

    pub(crate) fn current_user(&self) -> Result {
        let url = self.rest_api("user/current");
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn delete_restrictions(&self, id: &str) -> Result {
        let url = self.rest_api(&format!("content/{}/restriction", id));
        self.client
            .delete(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn get_restrictions_by_operation(&self, id: &str) -> Result {
        let url = self.rest_api(&format!("content/{}/restriction/byOperation", id));
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn move_page_relative(
        &self,
        page_id: &str,
        position: &str,
        target_id: &str,
    ) -> Result {
        let url = self.rest_api(&format!(
            "content/{}/move/{}/{}",
            page_id, position, target_id
        ));
        self.client
            .put(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }

    pub(crate) fn get_space_suggested_content_states(&self, space_key: &str) -> Result {
        let url = self.rest_api(&format!("space/{}/state", space_key));
        self.client
            .get(url)
            .basic_auth(self.api_user.clone(), Some(self.api_token.clone()))
            .header("Accept", "application/json")
            .header("X-Atlassian-Token", "no-check")
            .send()
    }
}
