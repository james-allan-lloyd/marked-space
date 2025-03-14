use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, File},
    io::{BufReader, Write},
    path::PathBuf,
};

use anyhow::{Context, Ok};
use reqwest::blocking::multipart::Part;
use serde_json::json;

use crate::{
    attachment::ImageAttachment,
    checksum::sha256_digest,
    confluence_client::ConfluenceClient,
    confluence_page::ConfluencePage,
    confluence_space::ConfluenceSpace,
    error::ConfluenceError,
    link_generator::LinkGenerator,
    markdown_page::{MarkdownPage, RenderedPage},
    markdown_space::MarkdownSpace,
    page_emojis::get_property_updates,
    responses::{self, Attachment, MultiEntityResult},
    sync_operation::{SyncOperation, SyncOperationResult},
    Result,
};

fn sync_page_attachments(
    confluence_client: &ConfluenceClient,
    page_id: &str,
    attachments: &[ImageAttachment],
) -> Result<()> {
    let existing_attachments: MultiEntityResult<Attachment> = confluence_client
        .get_attachments(page_id)?
        .error_for_status()?
        .json()?;

    let mut hashes = HashMap::<String, String>::new();
    let mut remove_titles_to_id = HashMap::<String, String>::new();
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
        remove_titles_to_id.insert(
            existing_attachment.title.clone(),
            existing_attachment.id.clone(),
        );
    }

    for attachment in attachments.iter() {
        // let filename: String = attachment.file_name().unwrap().to_str().unwrap().into();
        let attachment_name = attachment.name.clone();

        remove_titles_to_id.remove(&attachment_name);

        let op = SyncOperation::start(format!("[{}] attachment", attachment.path.display()), true);
        let input = File::open(&attachment.path)
            .with_context(|| format!("Opening attachment for {}", attachment_name))?;
        let reader = BufReader::new(input);
        let hashstring = sha256_digest(reader)?;
        if hashes.contains_key(&attachment_name)
            && hashstring == *hashes.get(&attachment_name).unwrap()
        {
            op.end(SyncOperationResult::Skipped);
            return Ok(());
        }

        let file_part = Part::file(&attachment.path)?.file_name(attachment.name.clone());

        let response =
            confluence_client.create_or_update_attachment(page_id, file_part, &hashstring)?;

        if !response.status().is_success() {
            // Handle non-2xx responses (e.g., 400 Bad Request)
            let status = response.status();
            let error_body = response.text()?;
            println!("Error! Status: {}, Body: {}", status, error_body);
        }

        op.end(SyncOperationResult::Updated);
    }

    let _remove_results: Vec<crate::confluence_client::Result> = remove_titles_to_id
        .iter()
        .map(|(title, id)| {
            let op = SyncOperation::start(format!("[{}] attachment", title), false);
            let result = confluence_client.remove_attachment(id);
            if result.is_ok() {
                op.end(SyncOperationResult::Deleted);
            } else {
                op.end(SyncOperationResult::Error);
            }
            result
        })
        .collect();

    Ok(())
}

fn sync_page_properties(
    confluence_client: &ConfluenceClient,
    page: &MarkdownPage,
    page_id: &str,
) -> Result<()> {
    let prop_json = confluence_client
        .get_properties(page_id)?
        .error_for_status()?
        .json::<MultiEntityResult<responses::ContentProperty>>()?;

    let property_updates = get_property_updates(page, &prop_json.results);

    for property_update in property_updates.iter() {
        let update_response = if property_update.value.is_null() {
            println!("Delete property {}", &property_update.key);
            confluence_client.delete_property(page_id, &property_update.id)
        } else if property_update.id.is_empty() {
            println!("Create property {}", &property_update.key);
            confluence_client.create_property(
                page_id,
                json!({"key": property_update.key, "value": property_update.value}),
            )
        } else {
            println!("Update property {}", &property_update.key);
            confluence_client.set_property(
                page_id,
                &property_update.id,
                json!({
                    "key": property_update.key,
                    "value": property_update.value,
                    "version": {
                        "message": property_update.version.message,
                        "number": property_update.version.number,
                    }
                }),
            )
        };

        update_response?.error_for_status()?;
    }

    Ok(())
}

// Returns the ID of the page that the content was synced to.
fn sync_page_content(
    confluence_client: &ConfluenceClient,
    space: &ConfluenceSpace,
    rendered_page: RenderedPage,
    existing_page: &ConfluencePage,
) -> Result<()> {
    let op = SyncOperation::start(
        format!("[{}] \"{}\"", rendered_page.source, rendered_page.title),
        true,
    );

    let parent_id = if rendered_page.is_home_page() {
        None
    } else if let Some(parent) = rendered_page.parent.clone() {
        Some(parent)
    } else {
        Some(space.homepage_id.clone())
    };

    let id = existing_page.id.clone();
    let version_message = rendered_page.version_message();
    if page_up_to_date(existing_page, &rendered_page, &parent_id, &version_message) {
        op.end(SyncOperationResult::Skipped);
        return Ok(());
    }

    let update_payload = json!({
        "id": id.clone(),
        "spaceId": space.id,
        "status": "current",
        "title": rendered_page.title,
        "parentId": parent_id,
        "body": {
            "representation": "storage",
            "value": rendered_page.content
        },
        "version": {
            "message": version_message,
            "number": existing_page.version.number + 1
        },
    });

    let resp = confluence_client.update_page(&id, update_payload)?;
    if !resp.status().is_success() {
        op.end(SyncOperationResult::Error);
        Err(ConfluenceError::failed_request(resp))
    } else {
        op.end(SyncOperationResult::Updated);
        Ok(())
    }
}

fn page_up_to_date(
    existing_page: &ConfluencePage,
    page: &RenderedPage,
    parent_id: &Option<String>,
    version_message: &String,
) -> bool {
    parent_id == &existing_page.parent_id
        && version_message == &existing_page.version.message
        && existing_page.title == page.title
}

pub fn sync_space<'a>(
    confluence_client: ConfluenceClient,
    markdown_space: &'a MarkdownSpace<'a>,
    output_dir: Option<String>,
) -> Result<()> {
    let space_key = markdown_space.key.clone();
    println!(
        "Parsing space {} from {} ...",
        space_key,
        markdown_space.dir.display()
    );
    let mut link_generator = LinkGenerator::new(&confluence_client.hostname, &markdown_space.key);

    let markdown_pages = markdown_space.parse(&mut link_generator)?;

    println!(
        "Synchronizing space {} on {}...",
        space_key, confluence_client.hostname
    );

    let mut space = ConfluenceSpace::get(&confluence_client, &space_key)?;
    space.read_all_pages(&confluence_client)?;
    space.link_pages(&mut link_generator, &markdown_space.dir);
    space.create_initial_pages(&mut link_generator, &confluence_client)?;

    for markdown_page in markdown_pages.iter() {
        let rendered_page = markdown_page.render(&link_generator)?;
        if let Some(ref d) = output_dir {
            output_content(d, &rendered_page)?;
        }
        let page_id = link_generator
            .get_file_id(&PathBuf::from(&rendered_page.source))
            .expect("All pages should have been created already.");
        let existing_page = space
            .get_existing_page(&page_id)
            .expect("Page should have been created already.");
        sync_page_content(&confluence_client, &space, rendered_page, &existing_page)?;
        sync_page_attachments(
            &confluence_client,
            &existing_page.id,
            &markdown_page.attachments,
        )?;
        sync_page_labels(
            &confluence_client,
            &existing_page.id,
            &markdown_page.front_matter.labels,
        )?;
        sync_page_properties(&confluence_client, markdown_page, &existing_page.id)?;
    }

    Ok(())
}

fn sync_page_labels(
    confluence_client: &ConfluenceClient,
    page_id: &str,
    labels: &[String],
) -> Result<()> {
    let mut label_set = HashSet::<String>::new();
    let body = labels
        .iter()
        .map(|label| {
            label_set.insert(label.clone());
            json!({"prefix": "", "name": label})
        })
        .collect::<Vec<serde_json::Value>>();

    let result = if !labels.is_empty() {
        confluence_client
            .set_page_labels(page_id, body)?
            .error_for_status()?
            .json::<MultiEntityResult<responses::Label>>()?
    } else {
        confluence_client
            .get_page_labels(page_id)?
            .error_for_status()?
            .json::<MultiEntityResult<responses::Label>>()?
    };

    let labels_removed = result
        .results
        .iter()
        .filter(|label| !label_set.contains(&label.name))
        .map(|label| {
            confluence_client
                .remove_label(page_id, label)?
                .error_for_status()?;

            Ok(label.name.clone())
        })
        .filter_map(|result| {
            result
                .map_err(|err: anyhow::Error| println!("{:#?}", err))
                .ok()
        })
        .collect::<Vec<String>>();

    if !labels_removed.is_empty() {
        println!("Removed labels: {}", labels_removed.join(","));
    }

    Ok(())
}

fn output_content(d: &String, page: &RenderedPage) -> Result<()> {
    let mut output_path = PathBuf::from(d);
    output_path.push(PathBuf::from(page.source.clone()).with_extension("xhtml"));
    if let Some(p) = output_path.parent() {
        create_dir_all(p)?;
    }
    println!("Writing to {}", output_path.display());
    File::create(output_path)?.write_all(page.content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{markdown_page::MarkdownPage, template_renderer::TemplateRenderer};

    use self::responses::Version;

    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild};
    use comrak::{nodes::AstNode, Arena};

    type TestResult = std::result::Result<(), anyhow::Error>;

    // TODO: we're really only testing the destination pathing here...
    fn parse_page(
        markdown_space: &MarkdownSpace,
        markdown_page_path: &Path,
        link_generator: &mut LinkGenerator,
    ) -> Result<RenderedPage> {
        // The returned nodes are created in the supplied Arena, and are bound by its lifetime.
        let mut template_renderer = TemplateRenderer::new(markdown_space)?;
        let arena = Arena::<AstNode>::new();
        let markdown_page = MarkdownPage::from_file(
            markdown_space,
            markdown_page_path,
            &arena,
            &mut template_renderer,
        )?;
        link_generator.register_markdown_page(&markdown_page)?;
        link_generator.register_confluence_page(&ConfluencePage {
            id: "29".to_string(),
            title: markdown_page.title.clone(),
            parent_id: None,
            version: Version {
                message: String::default(),
                number: 1,
            },
            path: None, // "foo.md".to_string(),
        });
        markdown_page.render(link_generator)
    }

    #[test]
    fn it_parses_page() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/markdown1.md")
            .write_str("# Page Title")
            .unwrap();

        let parsed_page = parse_page(
            &MarkdownSpace::from_directory(temp.child("test").path())?,
            temp.child("test/markdown1.md").path(),
            &mut LinkGenerator::default(),
        );

        assert!(parsed_page.is_ok());

        Ok(())
    }

    #[test]
    fn it_parses_page_titles_with_forward_slash() -> TestResult {
        let temp = assert_fs::TempDir::new()?;
        temp.child("test/markdown1.md")
            .write_str("# A title with a / slash")?;

        let parsed_page = parse_page(
            &MarkdownSpace::from_directory(temp.child("test").path())?,
            temp.child("test/markdown1.md").path(),
            &mut LinkGenerator::default(),
        );

        assert!(parsed_page.is_ok());

        let parsed_page = parsed_page.unwrap();

        assert_eq!(parsed_page.title, "A title with a / slash");
        Ok(())
    }

    #[test]
    fn it_uses_index_md_as_homepage() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str("# Page Title")
            .unwrap();

        let parsed_page = parse_page(
            &MarkdownSpace::from_directory(temp.child("test").path())?,
            temp.child("test/index.md").path(),
            &mut LinkGenerator::default(),
        )?;

        assert!(parsed_page.parent.is_none());

        Ok(())
    }

    #[test]
    fn it_uses_index_as_parent_for_subpages() -> TestResult {
        // Space
        // +-- Subpages Parent
        //     +-- Subpage Child
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str("# Homepage\nhomepage content")?;
        temp.child("test/subpages/index.md")
            .write_str("# Subpages Parent\nparent content")?;
        temp.child("test/subpages/child.md")
            .write_str("# Subpage Child\nchild content")?;
        let mut link_generator = LinkGenerator::default();
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let _home_page = parse_page(
            &space,
            temp.child("test/index.md").path(),
            &mut link_generator,
        )?;
        let _parent_page = parse_page(
            &space,
            temp.child("test/subpages/index.md").path(),
            &mut link_generator,
        )?;
        let child_page = parse_page(
            &space,
            temp.child("test/subpages/child.md").path(),
            &mut link_generator,
        )?;

        assert!(child_page.parent.is_some());
        assert_eq!(child_page.parent.unwrap(), "29");

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

    #[test]
    fn it_updates_title() -> TestResult {
        let confluence_page = ConfluencePage {
            id: String::from("1"),
            title: String::from("Old Title"),
            parent_id: None,
            version: Version {
                message: String::default(),
                number: 1,
            },
            path: None,
        };
        let rendered_page = RenderedPage {
            title: String::from("New title"),
            content: String::default(),
            source: String::default(),
            parent: None,
            checksum: String::default(),
        };

        assert!(!page_up_to_date(
            &confluence_page,
            &rendered_page,
            &None,
            &String::default()
        ));

        Ok(())
    }
}
