use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::{BufReader, Write},
    path::PathBuf,
};

use anyhow::Context;
use serde_json::json;

use crate::{
    checksum::sha256_digest,
    confluence_client::ConfluenceClient,
    confluence_page::ConfluencePage,
    confluence_space::ConfluenceSpace,
    error::ConfluenceError,
    html::LinkGenerator,
    markdown_page::RenderedPage,
    markdown_space::MarkdownSpace,
    responses::{
        self, Attachment, MultiEntityResult, PageBulkWithoutBody, PageSingleWithoutBody, Version,
    },
    Result,
};

fn sync_page_attachments(
    confluence_client: &ConfluenceClient,
    page_id: String,
    attachments: &[PathBuf],
) -> Result<()> {
    let existing_attachments: MultiEntityResult<Attachment> = confluence_client
        .get_attachments(&page_id)?
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
        let filename: String = attachment.file_name().unwrap().to_str().unwrap().into();

        remove_titles_to_id.remove(&filename);

        let op = SyncOperation::start(format!("[{}] attachment", filename), true);
        let input = File::open(attachment)
            .with_context(|| format!("Opening attachment for {}", filename))?;
        let reader = BufReader::new(input);
        let hashstring = sha256_digest(reader)?;
        if hashes.contains_key(&filename) && hashstring == *hashes.get(&filename).unwrap() {
            op.end(SyncOperationResult::Skipped);
            return Ok(());
        }

        let _resp = confluence_client
            .create_or_update_attachment(&page_id, attachment, &hashstring)?
            .error_for_status()?;
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

fn get_page_id_by_title(
    confluence_client: &ConfluenceClient,
    space_id: &str,
    title: &str,
) -> Result<Option<String>> {
    let resp = confluence_client
        .get_page_by_title(space_id, title, false)?
        .error_for_status()?;

    let content = resp.text()?;
    let existing_page: responses::MultiEntityResult<PageBulkWithoutBody> =
        serde_json::from_str(content.as_str())?;

    if existing_page.results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(existing_page.results[0].id.clone()))
    }
}

struct SyncOperation {
    desc: String,
    verbose: bool,
}

enum SyncOperationResult {
    Updated,
    Skipped,
    Created,
    Error,
    Deleted,
}

impl SyncOperation {
    fn start(desc: String, verbose: bool) -> SyncOperation {
        // print!("  {}", desc);
        SyncOperation { desc, verbose }
    }

    fn end(&self, result: SyncOperationResult) {
        let result_str = match result {
            SyncOperationResult::Updated => "Updated",
            SyncOperationResult::Skipped => "Skipped",
            SyncOperationResult::Created => "Created",
            SyncOperationResult::Error => "Error",
            SyncOperationResult::Deleted => "Deleted",
        };
        if self.verbose || !matches!(result, SyncOperationResult::Skipped) {
            println!("{}:  {}", self.desc, result_str);
        }
    }
}

// Returns the ID of the page that the content was synced to.
fn sync_page_content(
    confluence_client: &ConfluenceClient,
    space: &ConfluenceSpace,
    page: RenderedPage,
) -> Result<String> {
    let op = SyncOperation::start(format!("[{}] \"{}\"", page.source, page.title), true);

    let mut existing_page = if page.is_home_page() {
        Some(ConfluencePage::get_homepage(
            confluence_client,
            &space.homepage_id,
        )?)
    } else {
        space.get_existing_page(confluence_client, &page)?
    };

    let parent_id = if page.is_home_page() {
        None
    } else if let Some(parent) = page.parent.as_ref() {
        get_page_id_by_title(confluence_client, &space.id, parent)?
    } else {
        Some(space.homepage_id.clone())
    };

    let mut op_result = SyncOperationResult::Updated;
    if existing_page.is_none() {
        // it's important that we have a version message to make move detection
        // work, but you can't set the version string for a create call, so we
        // create a page with empty content, then update it with the new stuff.
        // Means we'll always have at least two versions.
        op_result = SyncOperationResult::Created;
        let resp = confluence_client.create_page(json!({
            "spaceId": space.id,
            "status": "current",
            "title": page.title.clone(),
            "parentId": parent_id,
        }))?;
        if !resp.status().is_success() {
            op.end(SyncOperationResult::Error);
            return Err(ConfluenceError::failed_request(resp));
        }

        let page: PageSingleWithoutBody = resp.json()?;
        existing_page = Some(ConfluencePage {
            id: page.id,
            title: page.title.clone(),
            parent_id: parent_id.clone(),
            version: Version {
                number: 1,
                message: String::default(),
            },
        });
    }

    let existing_page = existing_page.unwrap();
    let id = existing_page.id.clone();
    let version_message = page.version_message();
    if parent_id == existing_page.parent_id && version_message == existing_page.version.message
    // && page_up_to_date(&existing_page, &page)
    {
        op.end(SyncOperationResult::Skipped);
        return Ok(id);
    }

    let update_payload = json!({
        "id": id.clone(),
        "spaceId": space.id,
        "status": "current",
        "title": page.title,
        "parentId": parent_id,
        "body": {
            "representation": "storage",
            "value": page.content
        },
        "version": {
            "message": version_message,
            "number": existing_page.version.number + 1
        }
    });

    let resp = confluence_client.update_page(&id, update_payload)?;
    if !resp.status().is_success() {
        op.end(SyncOperationResult::Error);
        Err(ConfluenceError::failed_request(resp))
    } else {
        op.end(op_result);
        Ok(id)
    }
}

fn get_orphaned_pages(
    confluence_client: &ConfluenceClient,
    link_generator: &LinkGenerator,
    space_id: &str,
) -> Result<Vec<ConfluencePage>> {
    let pages = ConfluencePage::get_all(confluence_client, space_id)?;
    Ok(pages
        .into_iter()
        .filter(|confluence_page| {
            confluence_page
                .version
                .message
                .starts_with(ConfluencePage::version_message_prefix())
                && !link_generator.has_title(confluence_page.title.as_str())
        })
        .collect())
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
    let mut link_generator = LinkGenerator::new();

    let markdown_pages = markdown_space.parse(&mut link_generator)?;

    println!(
        "Synchronizing space {} on {}...",
        space_key, confluence_client.hostname
    );

    // let space = get_space(&confluence_client, space_key.as_str())?;
    let mut space = ConfluenceSpace::get(&confluence_client, &space_key)?;
    let orphaned_pages = get_orphaned_pages(&confluence_client, &link_generator, &space.id)?;
    orphaned_pages.iter().for_each(|p| {
        // todo: filter for orphans whose files still exist
        println!(
            "Orphaned page detected \"{}\", version comment: {}",
            p.title, p.version.message
        );
    });
    space.set_orphans(orphaned_pages);
    for markdown_page in markdown_pages.iter() {
        let page = markdown_page.render(&link_generator)?;
        if let Some(ref d) = output_dir {
            output_content(d, markdown_space, &page)?;
        }
        let page_id = sync_page_content(&confluence_client, &space, page)?;
        sync_page_attachments(&confluence_client, page_id, &markdown_page.attachments)?;
    }

    Ok(())
}

fn output_content(d: &String, markdown_space: &MarkdownSpace, page: &RenderedPage) -> Result<()> {
    let mut output_path = PathBuf::from(d);
    output_path.push(
        markdown_space
            .relative_page_path(&PathBuf::from(page.source.clone()).with_extension("xhtml"))?,
    );
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

    use crate::markdown_page::MarkdownPage;

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
        let arena = Arena::<AstNode>::new();
        let markdown_page = MarkdownPage::parse(markdown_space, markdown_page_path, &arena)?;
        link_generator.add_file_title(
            &PathBuf::from(markdown_page.source.clone()),
            &markdown_page.title,
        )?;
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
            &mut LinkGenerator::new(),
        );

        assert!(parsed_page.is_ok());

        Ok(())
    }

    fn _it_escapes_page_titles_with_forward_slash() -> TestResult {
        todo!()
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
            &mut LinkGenerator::new(),
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
        let mut link_generator = LinkGenerator::new();
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let _home_page = parse_page(
            &space,
            temp.child("test/index.md").path(),
            &mut link_generator,
        )?;
        let _parent_page = parse_page(
            &space,
            temp.child("test\\subpages\\index.md").path(),
            &mut link_generator,
        )?;
        let child_page = parse_page(
            &space,
            temp.child("test/subpages/child.md").path(),
            &mut link_generator,
        )?;

        println!("child {:#?}", child_page);

        assert!(child_page.parent.is_some());
        assert_eq!(child_page.parent.unwrap(), "Subpages Parent");

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

    fn _it_renames_default_parent_page_when_index_md_is_added() -> TestResult {
        todo!()
    }
}
