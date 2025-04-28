use std::path::PathBuf;

use crate::confluence_client::ConfluenceClient;
use crate::confluence_paginator::ConfluencePaginator;
use crate::console::print_status;
use crate::console::Status::Reordered;
use crate::link_generator::LinkGenerator;
use crate::markdown_page::MarkdownPage;
use crate::responses::Descendant;

use crate::error::Result;

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

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{confluence_client, error::TestResult, responses::Descendant};

    use super::sort_descendants;

    fn mock_move_page(
        server: &mut mockito::ServerGuard,
        page_id: &str,
        position: &str,
        target_id: &str,
    ) -> mockito::Mock {
        let url = format!(
            "/wiki/rest/api/content/{}/move/{}/{}",
            page_id, position, target_id
        );

        println!("{}", url);

        server
            .mock("PUT", url.as_str())
            .with_status(200)
            .with_header("authorization", "Basic Og==")
            .with_header("content-type", "application/json")
            .with_header("X-Atlassian-Token", "no-check")
            .create()
    }

    #[test]
    fn it_sorts_pages() -> TestResult {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();
        let client = confluence_client::ConfluenceClient::new_insecure(&host);

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

        let mock = mock_move_page(&mut server, "3", "after", "2");

        sort_descendants(&client, &all_descendants_data)?;

        mock.assert();

        Ok(())
    }
}
