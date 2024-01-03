use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::{BufReader, Write},
    path::PathBuf,
};

use regex::Regex;
use serde_json::json;

use crate::{
    checksum::sha256_digest,
    confluence_client::ConfluenceClient,
    confluence_page::ConfluencePage,
    error::ConfluenceError,
    html::LinkGenerator,
    markdown_page::RenderedPage,
    markdown_space::MarkdownSpace,
    responses::{self, Attachment, MultiEntityResult, PageBulkWithoutBody, PageSingle},
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
        let hashstring = sha256_digest(reader)?;
        if hashes.contains_key(&filename) && hashstring == *hashes.get(&filename).unwrap() {
            println!("Attachment {}: up to date", filename);
            return Ok(());
        }

        println!("Updating attachment");
        let _resp = confluence_client
            .create_or_update_attachment(&page_id, attachment, &hashstring)?
            .error_for_status()?;
    }

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
    println!("Content: {}", content);
    let existing_page: responses::MultiEntityResult<PageBulkWithoutBody> =
        serde_json::from_str(content.as_str())?;

    if existing_page.results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(existing_page.results[0].id.clone()))
    }
}

// Returns the ID of the page that the content was synced to.
fn sync_page_content(
    confluence_client: &ConfluenceClient,
    space: &responses::Space,
    page: RenderedPage,
) -> Result<String> {
    let mut payload = json!({
        "spaceId": space.id,
        "status": "current",
        "title": page.title,
        // "parentId": space.homepage_id,
        "body": {
            "representation": "storage",
            "value": page.content
        }
    });

    let existing_page = if page.is_home_page() {
        payload["parentId"] = serde_json::Value::Null;
        Some(ConfluencePage::get_homepage(
            confluence_client,
            &space.homepage_id,
        )?)
    } else {
        if let Some(parent) = page.parent.as_ref() {
            payload["parentId"] =
                get_page_id_by_title(confluence_client, &space.id, parent)?.into();
        } else {
            payload["parentId"] = space.homepage_id.clone().into();
        }
        ConfluencePage::get_page(confluence_client, space, &page)?
    };

    payload["parentId"] = if page.is_home_page() {
        serde_json::Value::Null
    } else if let Some(parent) = page.parent.as_ref() {
        get_page_id_by_title(confluence_client, &space.id, parent)?.into()
    } else {
        space.homepage_id.clone().into()
    };

    if existing_page.is_none() {
        println!("Page doesn't exist, creating");
        let resp = confluence_client.create_page(payload)?;
        if !resp.status().is_success() {
            return Err(ConfluenceError::failed_request(resp));
        } else {
            let page: PageSingle = resp.json()?;
            return Ok(page.id);
        }
    }

    let existing_page = existing_page.unwrap();

    let id = existing_page.id.clone();
    println!("Updating \"{}\" ({}) from {}", page.title, id, page.source);

    if page_up_to_date(&existing_page, &page) {
        println!("Page \"{}\": up to date", page.title);
        return Ok(id);
    }

    payload["id"] = id.clone().into();
    payload["version"] = json!({
        "message": "updated automatically",
        "number": existing_page.version_number + 1
    });

    let resp = confluence_client.update_page(&id, payload)?;
    if !resp.status().is_success() {
        Err(ConfluenceError::failed_request(resp))
    } else {
        Ok(id)
    }
}

fn page_up_to_date(existing_page: &ConfluencePage, page: &RenderedPage) -> bool {
    // TODO: avoid using the regex and other "cleanups" -> just has the content when you put it and compare the hashes of the next put
    let re = Regex::new(r#"\s*ri:version-at-save="\d+"\s*"#).unwrap();
    let existing_content = re.replace_all(existing_page.content.as_str(), "");
    let new_content = String::from(page.content.replace("<![CDATA[]]>", "").trim());
    existing_content == new_content
}

pub fn sync_space<'a>(
    confluence_client: ConfluenceClient,
    markdown_space: &'a MarkdownSpace<'a>,
    output_dir: Option<String>,
) -> Result<()> {
    let space_key = markdown_space.key.clone();
    println!("Parsing space {}...", space_key);
    let mut link_generator = LinkGenerator::new();

    let markdown_pages = markdown_space.parse(&mut link_generator)?;

    println!("Synchronizing space {}...", space_key);
    let space = get_space(&confluence_client, space_key.as_str())?;
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
        );
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
