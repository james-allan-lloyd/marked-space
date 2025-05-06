use std::collections::VecDeque;

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

/// A more complex than it should be sorting algorithm to get around the fact that the Confluence
/// API only supports relative moves.
///
/// When working through it (with a number of options), these were my goals:
/// - Actually result in a sorted child list on the server
/// - Minimize the number of calls to the server (hence we model the presumed server state so that
///   we don't have to move everything)
/// - Prefer to optimize for reordering children at the end (ie, new pages).
///
/// It has the worst performance when the unordered item is at the beginning.
fn sort_descendants<T: MoveContent>(
    all_descendants_data: &[Descendant],
    move_content: &mut T,
) -> Result<()> {
    if all_descendants_data.len() < 2 {
        return Ok(());
    }

    let mut server_state: VecDeque<&Descendant> = VecDeque::from_iter(all_descendants_data);

    // Create a simple sorted list
    let mut sorted_descendants = Vec::from(all_descendants_data);
    sorted_descendants.sort_by_key(|d| d.title.clone());

    let mut i = 0;

    if sorted_descendants[0].id != server_state[0].id {
        move_content.move_content(&sorted_descendants[0].id, "before", &server_state[0].id)?;
        print_status(Reordered, &sorted_descendants[0].title);
        let source_pos = server_state
            .iter()
            .position(|d| d.id == sorted_descendants[0].id)
            .unwrap();
        let descendant = server_state.remove(source_pos).unwrap();
        server_state.insert(0, descendant);
    }

    while i < sorted_descendants.len() - 1 {
        // println!("{:?}", server_state);
        assert_eq!(sorted_descendants[i].id, server_state[i].id);

        let current = &sorted_descendants[i];
        let next_sorted = &sorted_descendants[i + 1];
        let next_unsorted = &server_state[i + 1];
        if next_sorted.id != next_unsorted.id {
            // println!("i: {}, {} != {}", i + 1, next_sorted.id, next_unsorted.id);
            let after_target_id = &current.id;
            let page_id = &next_sorted.id;

            move_content.move_content(page_id, "after", after_target_id)?;
            print_status(Reordered, &server_state[i].title);

            let target_pos = i + 1;
            let source_pos = server_state
                .iter()
                .position(|d| d.id == sorted_descendants[i + 1].id)
                .unwrap();
            let descendant = server_state.remove(source_pos).unwrap();
            if target_pos < server_state.len() {
                // println!("insert {} {}", target_pos, sorted_descendants[i].id);
                // insert after
                server_state.insert(target_pos, descendant);
            } else {
                // println!("append");
                // insert at end
                server_state.push_back(descendant);
            }
        }

        i += 1;
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
        moves: Vec<(String, String, String)>,
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
            self.moves.push((
                String::from(content_id),
                String::from(_operation),
                String::from(target_id),
            ));

            println!("before {}: {:?}", _operation, self.result);
            let pos = self.result.iter().position(|id| id == content_id).unwrap();
            self.result.remove(pos);
            let target_pos = self.result.iter().position(|id| id == target_id).unwrap();
            match _operation {
                "after" => {
                    if target_pos + 1 < self.result.len() {
                        self.result.insert(target_pos + 1, content_id.into());
                    } else {
                        self.result.push(content_id.into());
                    }
                }
                "before" => {
                    println!("before pos: {} id: {}", target_pos, target_id);
                    self.result.insert(target_pos, content_id.into());
                }
                _ => todo!(),
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
        output_moves: Vec<(&str, &str, &str)>,
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

        let expected = output_moves
            .iter()
            .map(|(node, operation, target)| {
                (
                    String::from(*node),
                    String::from(*operation),
                    String::from(*target),
                )
            })
            .collect::<Vec<(String, String, String)>>();
        assert_eq!(
            expected, test_sorter.moves,
            "\nExpected moves: {:?} to sort {:?}\n  actual: {:?}",
            expected, input_order, test_sorter.moves
        );
        Ok(())
    }

    #[test]
    fn it_sorts_pages() -> TestResult {
        test_sort_descendants(vec!["3", "2"], vec![("2", "before", "3")])
    }

    #[test]
    fn it_only_moves_pages_that_were_added() -> TestResult {
        // adding is assumed to put them at the end. Should be only one move
        test_sort_descendants(vec!["0", "2", "3", "1"], vec![("1", "after", "0")])
    }

    #[test]
    fn it_only_moves_the_necessary_pages_ignoring_ordered_pages() -> TestResult {
        test_sort_descendants(
            vec!["0", "1", "2", "4", "3", "5"],
            vec![("3", "after", "2")],
        )
    }

    #[test]
    fn it_only_moves_the_necessary_pages_first_and_last() -> TestResult {
        test_sort_descendants(
            vec!["3", "0", "1", "2"],
            vec![
                ("0", "before", "3"),
                ("1", "after", "0"),
                ("2", "after", "1"),
            ],
        )
    }

    #[test]
    fn it_only_move_the_necessary_pages_last_to_first_mult() -> TestResult {
        test_sort_descendants(
            vec!["2", "3", "0", "1"],
            vec![("0", "before", "2"), ("1", "after", "0")],
        )
    }

    #[test]
    fn it_sorts_pages_with_multiple_required_moves() -> TestResult {
        // 0 3 2 1
        // 0 1 3 2
        // 0 1 2 3
        test_sort_descendants(
            vec!["3", "2", "0", "1"],
            vec![
                ("0", "before", "3"),
                ("1", "after", "0"),
                ("2", "after", "1"),
            ],
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

        let mock = test_server.mock_move_page("2", "before", "3");

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
