use std::path::{Component, PathBuf};

use clap::builder::OsStr;
use comrak::{nodes::AstNode, Arena};
use serde_json::json;

use crate::{
    confluence_client::ConfluenceClient, error::ConfluenceError, html::LinkGenerator,
    markdown_page::MarkdownPage, markdown_space::MarkdownSpace, responses, Result,
};

fn get_space(confluence_client: &ConfluenceClient, space_id: &str) -> Result<responses::Space> {
    let resp = match confluence_client.get_space_by_key(space_id) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(ConfluenceError::generic_error("Failed to get space id").into());
        }
    };

    if !resp.status().is_success() {
        return Err(ConfluenceError::failed_request(resp));
    }

    let json = resp.json::<serde_json::Value>()?;

    if json["results"].as_array().unwrap().is_empty() {
        return Err(ConfluenceError::generic_error(format!(
            "No such space: {}",
            space_id
        )));
    }

    match serde_json::from_value::<responses::Space>(json["results"][0].clone()) {
        Ok(parsed_space) => return Ok(parsed_space),
        Err(_) => return Err(ConfluenceError::generic_error("Failed to parse response.").into()),
    };
}

struct Page {
    title: String,
    content: String,
    source: String,
    destination: String,
}

fn render_page(
    space_key: &String,
    markdown_page: &MarkdownPage,
    link_generator: &LinkGenerator,
) -> Result<Page> {
    let content = markdown_page.to_html_string(link_generator)?.clone();
    let title = markdown_page.title.clone();
    let destination = if PathBuf::from(markdown_page.source.clone())
        .components()
        .last()
        == Some(Component::Normal(&OsStr::from("index.md")))
    {
        space_key.clone().to_uppercase()
    } else {
        format!("{}/{}", space_key.clone().to_uppercase(), title)
    };

    Ok(Page {
        title,
        content,
        source: markdown_page.source.clone(),
        destination,
    })
}

struct ConfluencePage {
    id: String,
    content: String,
    version_number: i32,
}

fn sync_page(
    confluence_client: &ConfluenceClient,
    space: &responses::Space,
    page: Page,
) -> Result<()> {
    println!("{}", page.content);
    let mut payload = json!({
        "spaceId": space.id,
        "status": "current",
        "title": page.title,
        "parentId": space.homepage_id,
        "body": {
            "representation": "storage",
            "value": page.content
        }
    });

    let existing_page = if page.destination == space.key {
        payload["parentId"] = serde_json::Value::Null;

        let existing_page = match confluence_client.get_page_by_id(&space.homepage_id) {
            Ok(resp) => resp,
            Err(_) => todo!(),
        };

        if !existing_page.status().is_success() {
            return Err(ConfluenceError::failed_request(existing_page));
        }

        println!("{:#?}", existing_page);
        let content = existing_page.text().unwrap_or_default();
        let json: responses::PageSingle = match serde_json::from_str(content.as_str()) {
            Ok(j) => j,
            Err(err) => {
                println!("{:#?}", content);
                return Err(ConfluenceError::generic_error(err.to_string()));
            }
        };

        let existing_content = match &json.body {
            responses::BodySingle::Storage(body) => body.value.clone(),
            responses::BodySingle::AtlasDocFormat(_) => todo!(),
            responses::BodySingle::View(_) => todo!(),
        };
        ConfluencePage {
            id: json.id,
            content: existing_content,
            version_number: json.version.number,
        }
    } else {
        let existing_page =
            match confluence_client.get_page_by_title(&space.id, page.title.as_str()) {
                Ok(resp) => resp,
                Err(_) => todo!(),
            };

        let json: responses::MultiEntityResult =
            match serde_json::from_str(existing_page.text().unwrap_or_default().as_str()) {
                Ok(j) => j,
                Err(_) => todo!(),
            };

        if json.results.is_empty() {
            println!("Page doesn't exist, creating");
            let resp = match confluence_client.create_page(payload) {
                Ok(r) => r,
                Err(_) => {
                    return Err(ConfluenceError::generic_error("Failed to create page").into())
                }
            };

            let status = resp.status();
            let content = resp.text().unwrap_or_default();
            if !status.is_success() {
                return Err(ConfluenceError::generic_error(format!(
                    "Failed to create page: {}",
                    content
                ))
                .into());
            } else {
                return Ok(()); // FIXME: return early
            }
        }

        let bulk_page = &json.results[0];
        let existing_content = match &bulk_page.body {
            responses::BodyBulk::Storage(body) => body.value.clone(),
            responses::BodyBulk::AtlasDocFormat(_) => todo!(),
        };
        ConfluencePage {
            id: bulk_page.id.clone(),
            content: existing_content,
            version_number: bulk_page.version.number,
        }
    };

    let id = existing_page.id.clone();
    println!("Updating \"{}\" ({}) from {}", page.title, id, page.source);

    // println!("body {:#?}", json.results[0].body);

    if existing_page.content == page.content {
        println!("Already up to date");
        return Ok(());
    }
    payload["id"] = id.clone().into();
    payload["version"] = json!({
        "message": "updated automatically",
        "number": existing_page.version_number + 1
    });

    let resp = match confluence_client.update_page(id, payload) {
        Ok(r) => r,
        Err(_) => return Err(ConfluenceError::generic_error("Failed to update page").into()),
    };

    if !resp.status().is_success() {
        return Err(ConfluenceError::generic_error(format!(
            "Failed to update page: {:?}",
            resp.text()
        ))
        .into());
    }

    println!("Page synced");
    Ok(())
}

pub fn sync_space(
    confluence_client: ConfluenceClient,
    markdown_space: &MarkdownSpace,
) -> Result<()> {
    let space_key = markdown_space.key.clone();
    println!("Parsing space {}...", space_key);
    let arena = Arena::<AstNode>::new();

    let mut parse_errors = Vec::<ConfluenceError>::default();

    let markdown_pages: Vec<MarkdownPage> = markdown_space
        .markdown_pages
        .iter()
        .map(|markdown_page_path| MarkdownPage::parse(markdown_page_path, &arena))
        .filter_map(|r| r.map_err(|e| parse_errors.push(e)).ok())
        .collect();

    if !parse_errors.is_empty() {
        let error_string: String = parse_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        return Err(ConfluenceError::generic_error(
            String::from("Error parsing space: ") + &error_string,
        ));
    }

    let mut link_generator = LinkGenerator::new();

    markdown_pages.iter().for_each(|markdown_page| {
        link_generator.add_file_title(
            markdown_space
                .relative_page_path(&PathBuf::from(markdown_page.source.clone()))
                .as_path(),
            &markdown_page.title,
        )
    });

    println!("Synchronizing space...");
    let space = get_space(&confluence_client, space_key.as_str())?;
    for markdown_page in markdown_pages.iter() {
        let page = render_page(&space_key, markdown_page, &link_generator)?;
        sync_page(&confluence_client, &space, page)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild};

    type TestResult = std::result::Result<(), ConfluenceError>;

    // TODO: we're really only testing the destination pathing here...
    fn parse_page(space_key: &String, markdown_page_path: &Path) -> Result<Page> {
        // The returned nodes are created in the supplied Arena, and are bound by its lifetime.
        let arena = Arena::<AstNode>::new();
        let markdown_page = MarkdownPage::parse(markdown_page_path, &arena)?;
        render_page(space_key, &markdown_page, &LinkGenerator::new())
    }

    #[test]
    fn it_parses_page() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        let _markdown1 = temp
            .child("test/markdown1.md")
            .write_str("# Page Title")
            .unwrap();

        let parsed_page = parse_page(
            &String::from("test"),
            temp.child("test/markdown1.md").path(),
        )?;

        assert_eq!(parsed_page.destination, "TEST/Page Title");

        Ok(())
    }

    #[test]
    fn it_escapes_page_titles_with_forward_slash() -> TestResult {
        Ok(())
    }

    #[test]
    fn it_uses_index_md_as_homepage() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        let _markdown1 = temp
            .child("test/index.md")
            .write_str("# Page Title")
            .unwrap();

        let parsed_page = parse_page(&String::from("test"), temp.child("test/index.md").path())?;

        assert_eq!(parsed_page.destination, "TEST");

        Ok(())
    }

    #[test]
    fn it_errors_when_not_able_to_parse_a_file() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        let _markdown1 = temp
            .child("test/index.md")
            .write_str("Missing title should cause error")
            .unwrap();

        let confluence_client = ConfluenceClient::new("host.example.com");
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let sync_result = sync_space(confluence_client, &space);

        assert!(sync_result.is_err());

        let expected_string = String::from("Failed to parse");
        let error_string = sync_result.unwrap_err().to_string();

        assert!(
            error_string.contains(&expected_string),
            "Unexpected error: [{}], should contain [{}]",
            error_string,
            expected_string
        );

        Ok(())
    }
}
