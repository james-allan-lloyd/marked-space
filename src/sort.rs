use std::path::PathBuf;

use crate::confluence_client::ConfluenceClient;
use crate::confluence_paginator::ConfluencePaginator;
use crate::console::print_status;
use crate::console::Status::Reordered;
use crate::link_generator::LinkGenerator;
use crate::markdown_page::MarkdownPage;
use crate::responses::Descendant;

use crate::error::Result;

#[derive(Debug, PartialEq, Eq)]
pub enum Sort {
    Incrementing,
    Unsorted, // or manual
}

impl Sort {
    pub fn from_str(sort_string: Option<&str>) -> Result<Sort> {
        if sort_string.is_none() {
            Ok(Sort::Unsorted)
        } else {
            let s = sort_string.unwrap();
            match s.to_ascii_lowercase().as_str() {
                "inc" => Ok(Sort::Incrementing),
                _ => Err(anyhow::anyhow!("invalid value")),
            }
        }
    }
}

pub fn sort_descendants(
    client: &ConfluenceClient,
    all_descendants_data: &[Descendant],
) -> Result<()> {
    if all_descendants_data.is_empty() {
        return Ok(());
    }
    let mut sorted_descendants = Vec::from(all_descendants_data);
    sorted_descendants.sort_by_key(|d| d.title.clone());

    for i in 1..sorted_descendants.len() {
        let page_id = &sorted_descendants[i].id;
        let next_page_id = &sorted_descendants[i - 1].id;
        print_status(Reordered, &sorted_descendants[i].title);
        client
            .move_page_relative(page_id, "after", next_page_id)?
            .error_for_status()?;
    }

    Ok(())
}

pub fn sync_sort(
    markdown_page: &MarkdownPage,
    link_generator: &LinkGenerator,
    confluence_client: &ConfluenceClient,
) -> Result<()> {
    let page_id = link_generator
        .get_file_id(&PathBuf::from(&markdown_page.source))
        .expect("Should all be created");

    if markdown_page.front_matter.sort != Sort::Unsorted {
        // TODO: should be able to construct this ourselves
        let response = if markdown_page.is_folder() {
            confluence_client.get_folder_descendants(page_id)?
        } else {
            confluence_client.get_page_descendants(page_id)?
        };

        let mut iter = ConfluencePaginator::<Descendant>::new(confluence_client);

        let all_descendants_data: Vec<Descendant> =
            iter.start(response)?.filter_map(|d| d.ok()).collect();

        sort_descendants(confluence_client, &all_descendants_data)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use mockito::Matcher;
    use serde_json::json;

    use crate::{
        confluence_client,
        confluence_page::{self, ConfluenceNode, ConfluencePageData},
        error::TestResult,
        link_generator::LinkGenerator,
        markdown_space::MarkdownSpace,
        responses::{self, ContentStatus, Descendant},
        sort::Sort,
    };

    use super::{sort_descendants, sync_sort};

    fn register_mark_and_conf_page<'a>(
        page_id: &str,
        link_generator: &mut LinkGenerator,
        markdown_page: crate::markdown_page::MarkdownPage<'a>,
    ) -> Result<crate::markdown_page::MarkdownPage<'a>, anyhow::Error> {
        link_generator.register_markdown_page(&markdown_page)?;
        link_generator.register_confluence_node(&ConfluenceNode {
            id: page_id.into(),
            title: markdown_page.title.clone(),
            parent_id: Some("99".into()),
            data: <confluence_page::ConfluenceNodeType>::from(ConfluencePageData {
                version: responses::Version {
                    number: 1,
                    message: ConfluencePageData::version_message_prefix().into(),
                },
                path: Some(PathBuf::from(&markdown_page.source)),
                status: ContentStatus::Current,
            }),
        });
        Ok(markdown_page)
    }

    struct TestServer {
        server: mockito::ServerGuard,
        client: confluence_client::ConfluenceClient,
    }

    impl Default for TestServer {
        fn default() -> Self {
            let server = mockito::Server::new();
            let host = server.host_with_port();
            let client = confluence_client::ConfluenceClient::new_insecure(&host);
            Self { server, client }
        }
    }

    impl TestServer {
        fn mock_descendants(
            &mut self,
            page_id: &str,
            all_descendants_data: &Vec<Descendant>,
        ) -> mockito::Mock {
            let url = format!("/wiki/api/v2/pages/{}/descendants", page_id);
            self.server
                .mock("GET", url.as_str())
                .match_query(Matcher::Any)
                .with_status(200)
                .with_header("authorization", "Basic Og==")
                .with_header("content-type", "application/json")
                .with_header("X-Atlassian-Token", "no-check")
                .with_body(json!({"results": all_descendants_data}).to_string())
                .create()
        }

        fn mock_move_page(
            &mut self,
            page_id: &str,
            position: &str,
            target_id: &str,
        ) -> mockito::Mock {
            let url = format!(
                "/wiki/rest/api/content/{}/move/{}/{}",
                page_id, position, target_id
            );

            self.server
                .mock("PUT", url.as_str())
                .with_status(200)
                .with_header("authorization", "Basic Og==")
                .with_header("content-type", "application/json")
                .with_header("X-Atlassian-Token", "no-check")
                .create()
        }
    }

    #[test]
    fn it_sorts_pages() -> TestResult {
        let mut test_server = TestServer::default();

        let all_descendants_data = vec![
            Descendant {
                id: "3".into(),
                title: "Page B".into(),
                _type: "page".into(),
                parent_id: "1".into(),
            },
            Descendant {
                id: "2".into(),
                title: "Page A".into(),
                _type: "page".into(),
                parent_id: "1".into(),
            },
        ];

        let mock = test_server.mock_move_page("3", "after", "2");

        sort_descendants(&test_server.client, &all_descendants_data)?;

        mock.assert();

        Ok(())
    }

    #[test]
    fn it_only_sorts_pages_with_sort_parameter_set() -> TestResult {
        let mut test_server = TestServer::default();

        let mut link_generator = LinkGenerator::default();

        let markdown_space = MarkdownSpace::default("test", &PathBuf::from("test"));
        let markdown_page = register_mark_and_conf_page(
            "1",
            &mut link_generator,
            markdown_space.page_from_str("index.md", "# Title\nContent")?,
        )?;

        let sorted_markdown_page = register_mark_and_conf_page(
            "2",
            &mut link_generator,
            markdown_space
                .page_from_str("index.md", "---\nsort: inc\n---\n# Sorted Title\nContent")?,
        )?;

        assert_eq!(sorted_markdown_page.front_matter.sort, Sort::Incrementing);

        let all_descendants_data = vec![
            Descendant {
                id: "3".into(),
                title: "Page B".into(),
                _type: "page".into(),
                parent_id: "1".into(),
            },
            Descendant {
                id: "2".into(),
                title: "Page A".into(),
                _type: "page".into(),
                parent_id: "1".into(),
            },
        ];

        test_server.mock_descendants("1", &all_descendants_data);
        test_server.mock_descendants("2", &all_descendants_data);

        let mock = test_server.mock_move_page("3", "after", "2");

        sync_sort(&markdown_page, &link_generator, &test_server.client)?;
        assert!(!mock.matched());

        sync_sort(&sorted_markdown_page, &link_generator, &test_server.client)?;
        assert!(mock.matched());

        Ok(())
    }
}
