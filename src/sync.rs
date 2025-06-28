use std::{
    collections::HashSet,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
};

use anyhow::Ok;
use serde_json::json;

use crate::{
    attachments::sync_page_attachments,
    confluence_client::ConfluenceClient,
    confluence_page::ConfluenceNode,
    confluence_space::ConfluenceSpace,
    console::{print_error, print_info, print_status, Status},
    error::ConfluenceError,
    folders::sync_folder,
    link_generator::LinkGenerator,
    markdown_page::{MarkdownPage, RenderedPage},
    markdown_space::MarkdownSpace,
    page_properties::sync_page_properties,
    page_statuses::sync_page_status,
    responses::{self, MultiEntityResult},
    restrictions::{sync_restrictions, RestrictionType},
    sort::sync_sort,
    sync_operation::SyncOperation,
    template_renderer::TemplateRenderer,
    Args, Result,
};

// Returns the ID of the page that the content was synced to.
fn sync_page_content(
    confluence_client: &ConfluenceClient,
    space: &ConfluenceSpace,
    rendered_page: RenderedPage,
    existing_node: &ConfluenceNode,
) -> Result<()> {
    let page_data = existing_node.page_data().unwrap();
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

    let id = existing_node.id.clone();
    let version_message = rendered_page.version_message();
    if page_up_to_date(existing_node, &rendered_page, &parent_id, &version_message) {
        op.end(Status::Skipped);
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
            "number": page_data.version.number + 1
        },
    });

    let resp = confluence_client.update_page(&id, update_payload)?;
    if !resp.status().is_success() {
        op.end(Status::Error);
        Err(ConfluenceError::failed_request(resp))
    } else {
        op.end(Status::Updated);
        Ok(())
    }
}

fn page_up_to_date(
    existing_node: &ConfluenceNode,
    page: &RenderedPage,
    parent_id: &Option<String>,
    version_message: &String,
) -> bool {
    parent_id == &existing_node.parent_id
        && existing_node.title == page.title
        && version_message == &existing_node.page_data().unwrap().version.message
}

pub fn sync_space<'a>(
    mut confluence_client: ConfluenceClient,
    markdown_space: &'a mut MarkdownSpace<'a>,
    args: Args,
) -> Result<()> {
    let space_key = markdown_space.key.clone();
    let space_dir = markdown_space.dir.clone();

    let mut template_renderer = TemplateRenderer::new(markdown_space, &confluence_client)?;
    let markdown_pages = markdown_space.parse(&mut template_renderer)?;

    let mut space = ConfluenceSpace::get(&confluence_client, &space_key)?;
    let mut link_generator =
        LinkGenerator::new(&confluence_client.hostname, &space_key, &space.homepage_id);

    for markdown_page in &markdown_pages {
        link_generator.register_markdown_page(markdown_page)?;
    }

    if args.single_editor {
        print_info("Using single editor restrictions")
    }

    if !args.check {
        print_info(&format!(
            "Synchronizing space {} on {}...",
            space_key, confluence_client.hostname
        ));
        let current_user: serde_json::Value = confluence_client
            .current_user()?
            .error_for_status()?
            .json()?;

        space.read_all_pages(&confluence_client)?;
        space.link_pages(&mut link_generator);
        space.archive_orphans(&link_generator, &space_dir, &confluence_client)?;
        space.restore_archived_pages(&link_generator, &confluence_client)?;
        space.create_initial_nodes(&mut link_generator, &confluence_client)?;
        for markdown_page in markdown_pages.iter() {
            if markdown_page.is_folder() {
                sync_folder(markdown_page, &link_generator, &space, &confluence_client)?;
            } else {
                sync_page(
                    markdown_page,
                    &mut link_generator,
                    &args,
                    &space,
                    &confluence_client,
                    &current_user,
                )?;
            }
            sync_sort(markdown_page, &link_generator, &mut confluence_client)?;
        }
    } else {
        print_info(&format!(
            "Checking space {} on {}...",
            space_key, confluence_client.hostname
        ));
        space.read_all_pages(&confluence_client)?;
        space.link_pages(&mut link_generator);
        for markdown_page in markdown_pages.iter() {
            let rendered_page = markdown_page.render(&link_generator)?;
            if let Some(ref d) = args.output {
                output_content(d, &rendered_page)?;
            }
        }
        print_info("Check complete");
    }

    Ok(())
}

fn sync_page(
    markdown_page: &MarkdownPage,
    link_generator: &mut LinkGenerator,
    args: &Args,
    space: &ConfluenceSpace,
    confluence_client: &ConfluenceClient,
    current_user: &tera::Value,
) -> Result<()> {
    let rendered_page = markdown_page.render(link_generator)?;
    if let Some(ref d) = args.output {
        output_content(d, &rendered_page)?;
    }
    let page_id = link_generator
        .get_file_id(&PathBuf::from(&rendered_page.source))
        .expect("error: All pages should have been created already.");
    let existing_page = space
        .get_existing_node(&page_id)
        .expect("error: Page should have been created already.");
    if existing_page.page_data().is_none() {
        return Err(anyhow::anyhow!("{} is not a page and cannot be converted (at this time). You'll need to delete it manually before marked-space can create it as a page", existing_page.title));
    }
    sync_page_content(confluence_client, space, rendered_page, &existing_page)?;
    sync_page_attachments(
        confluence_client,
        &existing_page.id,
        &markdown_page.source,
        &markdown_page.attachments,
        link_generator,
    )?;
    sync_page_labels(
        confluence_client,
        &existing_page.id,
        &markdown_page.front_matter.labels,
    )?;
    sync_page_status(
        confluence_client,
        markdown_page,
        link_generator,
        &space.content_states,
    )?;
    sync_page_properties(
        confluence_client,
        markdown_page,
        &existing_page.id,
        link_generator,
    )?;
    let restrictions_type = if args.single_editor {
        RestrictionType::SingleEditor(current_user)
    } else {
        RestrictionType::OpenSpace
    };
    sync_restrictions(restrictions_type, confluence_client, &existing_page)?;

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
                .map_err(|err: anyhow::Error| print_error(&format!("{:#?}", err)))
                .ok()
        })
        .collect::<Vec<String>>();

    if !labels_removed.is_empty() {
        print_status(
            Status::Deleted,
            &format!("labels: {}", labels_removed.join(",")),
        );
    }

    Ok(())
}

fn output_content(d: &String, page: &RenderedPage) -> Result<()> {
    let mut output_path = PathBuf::from(d);
    output_path.push(PathBuf::from(page.source.clone()).with_extension("xhtml"));
    if let Some(p) = output_path.parent() {
        create_dir_all(p)?;
    }
    print_info(&format!("Writing to {}", output_path.display()));
    File::create(output_path)?.write_all(page.content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData},
        markdown_page::MarkdownPage,
        template_renderer::TemplateRenderer,
    };

    use self::responses::Version;

    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild};
    use comrak::{nodes::AstNode, Arena};
    use responses::ContentStatus;

    type TestResult = std::result::Result<(), anyhow::Error>;

    // TODO: we're really only testing the destination pathing here...
    fn parse_page(
        markdown_space: &MarkdownSpace,
        markdown_page_path: &Path,
        link_generator: &mut LinkGenerator,
    ) -> Result<RenderedPage> {
        // The returned nodes are created in the supplied Arena, and are bound by its lifetime.
        let mut template_renderer = TemplateRenderer::default()?;
        let arena = Arena::<AstNode>::new();
        let markdown_page = MarkdownPage::from_file(
            &markdown_space.dir,
            markdown_page_path,
            &arena,
            &mut template_renderer,
        )?;
        link_generator.register_markdown_page(&markdown_page)?;
        link_generator.register_confluence_node(&ConfluenceNode {
            id: "29".to_string(),
            title: markdown_page.title.clone(),
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::default(),
                    number: 1,
                },
                path: None, // "foo.md".to_string(),
                status: ContentStatus::Current,
            }),
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
            &mut LinkGenerator::default_test(),
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
            &mut LinkGenerator::default_test(),
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
            &mut LinkGenerator::default_test(),
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
        let mut link_generator = LinkGenerator::default_test();
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
        let mut space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let sync_result = sync_space(confluence_client, &mut space, Args::default());

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
        let confluence_page = ConfluenceNode {
            id: String::from("1"),
            title: String::from("Old Title"),
            parent_id: None,

            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::default(),
                    number: 1,
                },
                path: None,
                status: ContentStatus::Current,
            }),
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
