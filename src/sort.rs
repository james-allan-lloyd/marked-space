use std::collections::HashMap;

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

trait MoveContent {
    fn move_content(&mut self, content_id: &str, operation: &str, target: &str) -> Result<()>;
}

impl MoveContent for ConfluenceClient {
    fn move_content(&mut self, content_id: &str, operation: &str, target: &str) -> Result<()> {
        self.move_page_relative(content_id, operation, target)?
            .error_for_status()?;
        Ok(())
    }
}

fn sort_descendants<T: MoveContent>(
    all_descendants_data: &[Descendant],
    move_content: &mut T,
) -> Result<()> {
    if all_descendants_data.is_empty() {
        return Ok(());
    }

    // Create a simple sorted list
    let mut sorted_descendants = Vec::from(all_descendants_data);
    sorted_descendants.sort_by_key(|d| d.title.clone());

    // Setup predecssors based on that sorted list
    let mut predecessors: HashMap<String, Option<String>> = HashMap::default();
    predecessors.insert(sorted_descendants[0].id.clone(), None);
    for i in 1..sorted_descendants.len() {
        let current_id = sorted_descendants[i].id.clone();
        let predecessor = sorted_descendants[i - 1].id.clone();
        predecessors.insert(current_id, Some(predecessor));
    }

    // If the predecessor is not the same as in the sorted list, tell confluence to update.
    // It probably still does too many updates for a single reorder; one to fix where the update
    // was, one to fix where it goes and one to fix the original item. Probably the move for the
    // original item is all that's actually necessary.
    let mut unsorted_predecessor = None;
    for unsorted_descendant in all_descendants_data {
        let page_id = &unsorted_descendant.id;
        let sorted_predecessor = predecessors[page_id].clone();
        if sorted_predecessor != unsorted_predecessor && sorted_predecessor.is_some() {
            print_status(Reordered, &unsorted_descendant.title);
            move_content.move_content(page_id, "after", sorted_predecessor.as_ref().unwrap())?;
            // unsorted_predecessor remains as the current_id
        } else {
            unsorted_predecessor = Some(page_id.to_owned());
        }
    }

    Ok(())
}

pub fn sync_sort(
    markdown_page: &MarkdownPage,
    link_generator: &LinkGenerator,
    confluence_client: &mut ConfluenceClient,
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

        sort_descendants(&all_descendants_data, confluence_client)?;
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

    use super::{sort_descendants, sync_sort, MoveContent};

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

    struct TestSorter {
        moves: Vec<(String, String)>,
        result: Vec<String>,
    }

    impl TestSorter {
        fn create(input_order: &Vec<&str>) -> Self {
            Self {
                moves: Vec::default(),
                result: input_order.iter().map(|s| String::from(*s)).collect(),
            }
        }
    }

    impl MoveContent for TestSorter {
        fn move_content(
            &mut self,
            content_id: &str,
            _operation: &str,
            target_id: &str,
        ) -> crate::error::Result<()> {
            self.moves
                .push((String::from(content_id), String::from(target_id)));

            println!("before: {:?}", self.result);
            let pos = self.result.iter().position(|id| id == content_id).unwrap();
            let target_pos = self.result.iter().position(|id| id == target_id).unwrap();
            self.result.remove(pos);
            if target_pos + 1 < self.result.len() {
                self.result.insert(target_pos + 1, content_id.into());
            } else {
                self.result.push(content_id.into());
            }

            println!("after: {:?}", self.result);

            Ok(())
        }
    }

    fn is_sorted<T: PartialOrd>(vec: &[T]) -> bool {
        vec.windows(2).all(|window| window[0] <= window[1])
    }

    fn test_sort_descendants(
        input_order: Vec<&str>,
        output_moves: Vec<(&str, &str)>,
    ) -> TestResult {
        let mut test_sorter = TestSorter::create(&input_order);
        let all_descendants_data = input_order
            .iter()
            .map(|i| Descendant {
                id: String::from(*i),
                title: format!("Page {}", i),
                _type: "page".into(),
                parent_id: "99".into(),
            })
            .collect::<Vec<Descendant>>();
        sort_descendants(&all_descendants_data, &mut test_sorter)?;
        assert!(
            is_sorted(&test_sorter.result),
            "Not sorted: {:?}",
            &test_sorter.result
        );
        assert_eq!(
            output_moves
                .iter()
                .map(|(node, target)| (String::from(*node), String::from(*target)))
                .collect::<Vec<(String, String)>>(),
            test_sorter.moves,
        );
        Ok(())
    }

    #[test]
    fn it_sorts_pages() -> TestResult {
        test_sort_descendants(vec!["3", "2"], vec![("3", "2")])
    }

    #[test]
    fn it_only_moves_the_necessary_pages() -> TestResult {
        test_sort_descendants(vec!["0", "3", "1", "2"], vec![("3", "2")])
    }

    #[test]
    fn it_only_moves_the_necessary_pages_first_and_last() -> TestResult {
        test_sort_descendants(vec!["3", "0", "1", "2"], vec![("3", "2")])
    }

    #[test]
    fn it_only_move_the_necessary_pages_last_to_first_mult() -> TestResult {
        test_sort_descendants(vec!["2", "3", "0", "1"], vec![("2", "1"), ("3", "2")])
    }

    #[test]
    fn it_sorts_pages_with_multiple_required_moves() -> TestResult {
        test_sort_descendants(
            vec!["3", "2", "0", "1"],
            vec![("3", "2"), ("2", "1"), ("3", "2")],
        )
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

        sync_sort(&markdown_page, &link_generator, &mut test_server.client)?;
        assert!(!mock.matched());

        sync_sort(
            &sorted_markdown_page,
            &link_generator,
            &mut test_server.client,
        )?;
        assert!(mock.matched());

        Ok(())
    }
}
