use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::{BufReader, Read, Write},
    path::{Component, PathBuf},
};

use data_encoding::HEXUPPER;
use ring::digest::{Context, Digest, SHA256};

use clap::builder::OsStr;
use comrak::{nodes::AstNode, Arena};
use regex::Regex;
use serde_json::json;

use crate::{
    confluence_client::ConfluenceClient,
    error::ConfluenceError,
    html::LinkGenerator,
    markdown_page::MarkdownPage,
    markdown_space::MarkdownSpace,
    responses::{self, Attachment, MultiEntityResult, PageBulk, PageSingle},
    Result,
};

fn get_space(confluence_client: &ConfluenceClient, space_id: &str) -> Result<responses::Space> {
    let resp = match confluence_client.get_space_by_key(space_id) {
        Ok(resp) => resp,
        Err(_) => {
            return Err(ConfluenceError::generic_error("Failed to get space id"));
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
        Ok(parsed_space) => Ok(parsed_space),
        Err(_) => Err(ConfluenceError::generic_error("Failed to parse response.")),
    }
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

fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    Ok(context.finish())
}

fn sync_page_attachments(
    confluence_client: &ConfluenceClient,
    page_id: String,
    attachments: &Vec<PathBuf>,
) -> Result<()> {
    let existing_attachments: MultiEntityResult<Attachment> = confluence_client
        .get_attachments(&page_id)?
        .error_for_status()?
        .json()?;

    let mut hashes = HashMap::<String, String>::new();
    for existing_attachment in existing_attachments.results.iter() {
        if existing_attachment.comment.starts_with("hash:") {
            hashes.insert(
                existing_attachment.title.clone(),
                existing_attachment
                    .comment
                    .strip_prefix("hash:")
                    .unwrap()
                    .into(),
            );
        }
    }

    for attachment in attachments.iter() {
        let filename: String = attachment.file_name().unwrap().to_str().unwrap().into();
        let input = File::open(attachment)?;
        let reader = BufReader::new(input);
        let hash = sha256_digest(reader)?;
        let hashstring = HEXUPPER.encode(hash.as_ref());
        if hashes.contains_key(&filename) {
            if hashstring == *hashes.get(&filename).unwrap() {
                println!("Attachment {}: up to date", filename);
                return Ok(());
            }
        }

        println!("Updating attachment");
        let _resp = confluence_client
            .create_or_update_attachment(&page_id, attachment, &hashstring)?
            .error_for_status()?;
    }

    Ok(())
}

// Returns the ID of the page that the content was synced to.
fn sync_page_content(
    confluence_client: &ConfluenceClient,
    space: &responses::Space,
    page: Page,
) -> Result<String> {
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

        let existing_page: responses::PageSingle = confluence_client
            .get_page_by_id(&space.homepage_id)?
            .error_for_status()?
            .json()?;

        let existing_content = match &existing_page.body {
            responses::BodySingle::Storage(body) => body.value.clone(),
            responses::BodySingle::AtlasDocFormat(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "atlas doc format".into(),
                }
                .into())
            }
            responses::BodySingle::View(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "view format".into(),
                }
                .into())
            }
        };
        ConfluencePage {
            id: existing_page.id,
            content: existing_content,
            version_number: existing_page.version.number,
        }
    } else {
        let existing_page: responses::MultiEntityResult<PageBulk> = confluence_client
            .get_page_by_title(&space.id, page.title.as_str())?
            .error_for_status()?
            .json()?;

        if existing_page.results.is_empty() {
            println!("Page doesn't exist, creating");
            let resp = confluence_client.create_page(payload)?;
            if !resp.status().is_success() {
                return Err(ConfluenceError::failed_request(resp));
            } else {
                let page: PageSingle = resp.json()?;
                return Ok(page.id);
            }
        }

        let bulk_page = &existing_page.results[0];
        let existing_content = match &bulk_page.body {
            responses::BodyBulk::Storage(body) => body.value.clone(),
            responses::BodyBulk::AtlasDocFormat(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "atlas doc format".into(),
                }
                .into())
            }
        };
        ConfluencePage {
            id: bulk_page.id.clone(),
            content: existing_content,
            version_number: bulk_page.version.number,
        }
    };

    let id = existing_page.id.clone();
    println!("Updating \"{}\" ({}) from {}", page.title, id, page.source);

    let re = Regex::new(r#"\s*ri:version-at-save="\d+"\s*"#).unwrap();
    let existing_content = re.replace_all(existing_page.content.as_str(), "");
    let new_content = String::from(page.content.replace("<![CDATA[]]>", "").trim());
    // File::create(format!("{}-existing.xhtml", id))?.write_all(existing_content.as_bytes())?;
    // File::create(format!("{}-new.xhtml", id))?.write_all(page.content.as_bytes())?;
    if existing_content == new_content {
        println!("Page \"{}\": up to date", page.title);
        return Ok(id);
    }
    payload["id"] = id.clone().into();
    payload["version"] = json!({
        "message": "updated automatically",
        "number": existing_page.version_number + 1
    });

    let resp = confluence_client.update_page(&id, payload)?;
    // .error_for_status()?;
    if !resp.status().is_success() {
        Err(ConfluenceError::failed_request(resp))
    } else {
        Ok(id)
    }
}

pub fn sync_space(
    confluence_client: ConfluenceClient,
    markdown_space: &MarkdownSpace,
    output_dir: Option<String>,
) -> Result<()> {
    let space_key = markdown_space.key.clone();
    println!("Parsing space {}...", space_key);
    let arena = Arena::<AstNode>::new();

    let mut parse_errors = Vec::<anyhow::Error>::default();

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

    println!("Synchronizing space {}...", space_key);
    let space = get_space(&confluence_client, space_key.as_str())?;
    for markdown_page in markdown_pages.iter() {
        let page = render_page(&space_key, markdown_page, &link_generator)?;
        if let Some(ref d) = output_dir {
            let mut output_path = PathBuf::from(d);
            output_path.push(
                markdown_space.relative_page_path(
                    &PathBuf::from(page.source.clone()).with_extension("xhtml"),
                ),
            );
            if let Some(p) = output_path.parent() {
                create_dir_all(p)?;
            }
            println!("Writing to {}", output_path.display());

            File::create(output_path)?.write_all(page.content.as_bytes())?;
            // .context("writing confluence output")?;
        }
        let page_id = sync_page_content(&confluence_client, &space, page)?;
        sync_page_attachments(&confluence_client, page_id, &markdown_page.attachments)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild};

    type TestResult = std::result::Result<(), anyhow::Error>;

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
        temp.child("test/markdown1.md")
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
        temp.child("test/index.md")
            .write_str("# Page Title")
            .unwrap();

        let parsed_page = parse_page(&String::from("test"), temp.child("test/index.md").path())?;

        assert_eq!(parsed_page.destination, "TEST");

        Ok(())
    }

    #[test]
    fn it_errors_when_not_able_to_parse_a_file() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str("Missing title should cause error")
            .unwrap();

        let confluence_client = ConfluenceClient::new("host.example.com");
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let sync_result = sync_space(confluence_client, &space, None);

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
