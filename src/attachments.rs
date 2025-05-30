use crate::{
    checksum::sha256_digest,
    confluence_paginator::ConfluencePaginator,
    console::{print_error, Status},
    error::Result,
    responses::{Attachment, Content},
    sync_operation::SyncOperation,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use comrak::nodes::NodeLink;
use regex::Regex;
use reqwest::blocking::multipart::Part;

use crate::{
    confluence_client::ConfluenceClient,
    confluence_storage_renderer::{escape_href, WriteWithLast},
    link_generator::LinkGenerator,
    responses::MultiEntityResult,
};

#[derive(Debug, PartialEq)]
pub struct ImageAttachment {
    pub url: String,   // how this was specified in the markdown
    pub path: PathBuf, // the full path to the file
    pub name: String,  // a simple name
}

impl ImageAttachment {
    pub fn new(url: &str, page_path: &Path) -> Self {
        let mut path = PathBuf::from(page_path);
        path.push(url);

        ImageAttachment {
            path,
            url: String::from(url),
            name: link_to_name(url),
        }
    }
}

fn link_to_name(url: &str) -> String {
    let re = Regex::new(r"[/\\]").unwrap();
    re.replace_all(url, "_").into()
}

pub fn render_link_enter(nl: &NodeLink, output: &mut WriteWithLast) -> io::Result<()> {
    output.write_all(br#"<ac:image ac:align="center""#)?;
    if !nl.title.is_empty() {
        output.write_all(format!(" ac:title=\"{}\"", nl.title).as_bytes())?;
    }
    output.write_all(b">")?;
    if nl.url.contains("://") {
        output.write_all(b"<ri:url ri:value=\"")?;
        escape_href(output, nl.url.as_bytes())?;
    } else {
        output.write_all(b"<ri:attachment ri:filename=\"")?;
        let url = link_to_name(&nl.url);
        output.write_all(url.as_bytes())?;
    }

    output.write_all(b"\"/>")?;

    Ok(())
}

pub fn render_link_leave(_nl: &NodeLink, output: &mut WriteWithLast) -> io::Result<()> {
    output.write_all(b"</ac:image>")?;
    Ok(())
}

pub fn sync_page_attachments(
    confluence_client: &ConfluenceClient,
    page_id: &str,
    page_source: &str,
    attachments: &[ImageAttachment],
    link_generator: &mut LinkGenerator,
) -> Result<()> {
    let existing_attachments: MultiEntityResult<Attachment> = confluence_client
        .get_attachments(page_id)?
        .error_for_status()?
        .json()?;

    let mut hashes = HashMap::<String, String>::new();
    let mut remove_titles_to_id = HashMap::<String, String>::new();
    let mut title_to_fileid = HashMap::<String, String>::new();
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
        title_to_fileid.insert(
            existing_attachment.title.clone(),
            existing_attachment.file_id.clone(),
        );
    }

    for attachment in attachments.iter() {
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
            // still add the existing attachment to lookup for covers
            let id = title_to_fileid[&attachment_name].clone();
            link_generator.register_attachment_id(page_source, &attachment.url, &id);
            op.end(Status::Skipped);
            return Ok(());
        }

        let file_part = Part::file(&attachment.path)?.file_name(attachment.name.clone());

        let response =
            confluence_client.create_or_update_attachment(page_id, file_part, &hashstring)?;

        if !response.status().is_success() {
            // Handle non-2xx responses (e.g., 400 Bad Request)
            let status = response.status();
            let error_body = response.text()?;
            print_error(&format!(
                "error updating attachment: Status: {}, Body: {}",
                status, error_body
            ));
        } else {
            let results: Vec<Content> = ConfluencePaginator::<Content>::new(confluence_client)
                .start(response)?
                .filter_map(|f| f.ok())
                .collect();

            assert_eq!(results.len(), 1);
            assert_eq!(results[0].title, attachment_name);
            let id = results[0].extensions["fileId"].as_str().unwrap();
            // add new attachment to lookup
            link_generator.register_attachment_id(page_source, &attachment.url, id);
        }

        op.end(Status::Updated);
    }

    let _remove_results: Vec<crate::confluence_client::Result> = remove_titles_to_id
        .iter()
        .map(|(title, id)| {
            let op = SyncOperation::start(format!("[{}] attachment", title), false);
            let result = confluence_client.remove_attachment(id);
            if result.is_ok() {
                op.end(Status::Deleted);
            } else {
                op.end(Status::Error);
            }
            result
        })
        .collect();

    Ok(())
}

#[cfg(test)]
mod test {
    use std::{io::Cursor, path::PathBuf};

    use comrak::nodes::NodeLink;

    use crate::{confluence_storage_renderer::WriteWithLast, error::TestResult};

    use super::*;

    #[test]
    fn it_renders_node() -> TestResult {
        let nl = NodeLink {
            url: String::from("image.png"),
            title: String::from("some title"),
        };

        let mut cursor = Cursor::new(vec![0; 15]);
        let mut output = WriteWithLast::from_write(&mut cursor);
        render_link_enter(&nl, &mut output)?;
        render_link_leave(&nl, &mut output)?;

        assert_eq!(String::from_utf8(cursor.into_inner()).unwrap(),
            "<ac:image ac:align=\"center\" ac:title=\"some title\"><ri:attachment ri:filename=\"image.png\"/></ac:image>"
        );

        Ok(())
    }

    #[test]
    fn it_renders_image_link_in_subdirectory() -> TestResult {
        let nl = NodeLink {
            url: String::from("assets/image.png"),
            title: String::from("some title"),
        };

        let mut cursor = Cursor::new(vec![0; 15]);
        let mut output = WriteWithLast::from_write(&mut cursor);
        render_link_enter(&nl, &mut output)?;
        render_link_leave(&nl, &mut output)?;

        assert_eq!(String::from_utf8(cursor.into_inner()).unwrap(),
            "<ac:image ac:align=\"center\" ac:title=\"some title\"><ri:attachment ri:filename=\"assets_image.png\"/></ac:image>"
        );

        Ok(())
    }

    #[test]
    fn it_renders_image_link_in_subdirectories() {
        // Cannot upload files with names that contain slashes... confluence will strip the
        // directories and you'll end up with a broken link in the confluence page. Instead we
        // replace slashes with underscore.
        let image_name = link_to_name("./assets/image.png");

        assert_eq!(image_name, "._assets_image.png");
    }

    #[test]
    fn it_makes_absolute_path() -> TestResult {
        let image_url = String::from("./assets/image.png");
        let page_path = PathBuf::from("/tmp/foo/bar");
        let attachment = ImageAttachment::new(&image_url, &page_path);

        assert_eq!(
            attachment.path,
            PathBuf::from("/tmp/foo/bar/assets/image.png")
        );

        Ok(())
    }
}
