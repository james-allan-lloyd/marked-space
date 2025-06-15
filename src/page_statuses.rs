use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::{
    confluence_client::ConfluenceClient, error::Result, link_generator::LinkGenerator,
    markdown_page::MarkdownPage, responses,
};

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Hash)]
pub enum PageStatus {
    #[serde(rename = "draft")]
    RoughDraft,
    #[serde(rename = "in-progress")]
    InProgress,
    #[serde(rename = "ready")]
    ReadyForReview,
    #[serde(rename = "verified")]
    Verified,
}

#[derive(Debug)]
pub struct ContentStates {
    states: HashMap<PageStatus, responses::ContentState>,
}
impl ContentStates {
    pub fn new(content_states: &[responses::ContentState]) -> Self {
        let standard_states = vec![
            (PageStatus::RoughDraft, "Rough draft"),
            (PageStatus::InProgress, "In progress"),
            (PageStatus::ReadyForReview, "Ready for review"),
            (PageStatus::Verified, "Verified"),
        ];
        let mut states = HashMap::new();
        for (status, status_name) in standard_states {
            if let Some(content_state) = content_states.iter().find(|x| x.name == status_name) {
                states.insert(status, content_state.clone());
            }
        }
        Self { states }
    }

    pub fn to_confluence_json(&self, page_status: &PageStatus) -> Result<serde_json::Value> {
        if let Some(content_state) = self.states.get(page_status) {
            Ok(serde_json::to_value(content_state)?)
        } else {
            Err(anyhow!("Don't have an ID for page status"))
        }
    }
}

pub fn sync_page_status(
    client: &ConfluenceClient,
    markdown_page: &MarkdownPage,
    link_generator: &LinkGenerator,
    content_states: &ContentStates,
) -> Result<()> {
    if let Some(content_status) = &markdown_page.front_matter.status {
        client
            .set_content_state(
                &link_generator
                    .get_file_id(&PathBuf::from(&markdown_page.source))
                    .expect("Should have id for file"),
                "current",
                content_states.to_confluence_json(content_status)?,
            )?
            .error_for_status()?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use serde_json::json;

    use crate::{
        confluence_client::ConfluenceClient, error::TestResult, frontmatter::FrontMatter,
        link_generator::LinkGenerator, markdown_space::MarkdownSpace, page_statuses::ContentStates,
        responses, test_helpers::register_mark_and_conf_page,
    };

    use super::{sync_page_status, PageStatus};

    #[test]
    fn it_returns_none_with_no_status() -> TestResult {
        let fm = saphyr_serde::de::from_str::<FrontMatter>("").expect("Should Deserialize");
        assert_eq!(fm.status, None);
        Ok(())
    }

    fn it_returns_status(
        front_matter_string: &str,
        expected_status: PageStatus,
        confluence_content_state_name: &str,
    ) -> TestResult {
        let states = serde_json::from_value::<Vec<responses::ContentState>>(
            json!([{"id":13500442,"color":"#ffc400","name":confluence_content_state_name}]),
        )
        .unwrap();
        let content_states = ContentStates::new(&states);
        let fm = saphyr_serde::de::from_str::<FrontMatter>(front_matter_string)?;
        assert_eq!(fm.status, Some(expected_status));

        let prop = content_states.to_confluence_json(&fm.status.unwrap())?;
        assert_eq!(prop["name"], states[0].name);
        assert_eq!(prop["id"], states[0].id);
        assert_eq!(prop["color"], states[0].color);
        Ok(())
    }

    #[test]
    fn it_returns_rough_draft() -> TestResult {
        it_returns_status("status: draft", PageStatus::RoughDraft, "Rough draft")
    }

    #[test]
    fn it_returns_in_progress() -> TestResult {
        it_returns_status("status: in-progress", PageStatus::InProgress, "In progress")
    }

    #[test]
    fn it_returns_ready() -> TestResult {
        it_returns_status(
            "status: ready",
            PageStatus::ReadyForReview,
            "Ready for review",
        )
    }

    #[test]
    fn it_returns_verified() -> TestResult {
        it_returns_status("status: verified", PageStatus::Verified, "Verified")
    }

    #[test]
    fn it_raises_error_if_unknown_status() {
        let result = it_returns_status("status: foobarbaz", PageStatus::InProgress, "In progress");
        let error = result.expect_err("Should return error for unknown error, but didn't fail");
        let expected = "unknown variant `foobarbaz`, expected one of";
        let actual = format!("{}", error);
        assert!(
            actual.contains(expected),
            "\nExpected\n\n  {}\n\nto contain\n\n  {}\n",
            actual,
            expected
        );
    }

    #[test]
    fn it_creates_page_status() -> TestResult {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();

        let mock = mock_set_content_state(&mut server);

        let response = json!([{"id":13500442,"color":"#ffc400","name":"Rough draft"}]); // ,{"id":13500443,"color":"#2684ff","name":"In progress"},{"id":13500444,"color":"#57d9a3","name":"Ready for review"},{"id":37912577,"color":"#1d7afc","name":"Verified"}]);
        let states = serde_json::from_value::<Vec<responses::ContentState>>(response).unwrap();
        let content_states = ContentStates::new(&states);

        let markdown_space = MarkdownSpace::default("test", &PathBuf::from("test"));
        let mut link_generator = LinkGenerator::default_test();
        let markdown_page = register_mark_and_conf_page(
            "1",
            &mut link_generator,
            markdown_space
                .page_from_str("index.md", "---\nstatus: draft\n---\n# Title\nContent")?,
        )?;

        let client = ConfluenceClient::new_insecure(&host);
        sync_page_status(&client, &markdown_page, &link_generator, &content_states)?;

        mock.assert();
        Ok(())
    }

    fn mock_set_content_state(server: &mut mockito::ServerGuard) -> mockito::Mock {
        server
            .mock("PUT", "/wiki/rest/api/content/1/state")
            .match_query(mockito::Matcher::UrlEncoded(
                "status".into(),
                "current".into(),
            ))
            .with_status(200)
            .with_header("authorization", "Basic Og==")
            .with_header("content-type", "application/json")
            .with_header("X-Atlassian-Token", "no-check")
            .with_body(
                json!({
                  "contentState": {
                    "id": 1,
                    "name": "<string>",
                    "color": "<string>"
                  },
                  "lastUpdated": "<string>"
                })
                .to_string(),
            )
            .expect(1) // only called once
            .create()
    }
}
