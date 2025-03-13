use std::{io::{self, Write}, path::{Path, PathBuf}};

use comrak::nodes::NodeLink;
use anyhow::anyhow;

use crate::confluence_storage_renderer::{escape_href, WriteWithLast};


pub struct ImageAttachment {
    pub url: String,  // how this was specified in the markdown
    pub path: PathBuf,
}

impl ImageAttachment {
    
    fn new(url: &str, page_path: &Path) -> Self {
        let mut path = PathBuf::from(page_path);
        path.push(url);

        ImageAttachment {
            path,
            url: String::from(url)
        }
    }
}

pub fn attachment_from(image_url: &str, page_path: &Path) -> PathBuf {
    let mut attachment_path = PathBuf::from(page_path);
    attachment_path.push(image_url);
    attachment_path
}

pub fn attachment_name(image_path: &Path, page_path: &Path) -> anyhow::Result<String> {
    // if let Ok(relative_path) = image_path.strip_prefix(page_path) {
    //     Ok(relative_path.to_str().unwrap().into())
    // }
    // else {
    //     Err(anyhow!("Missing prefix {} from {}", page_path.display(), image_path.display()))
    // }
    Ok(image_path.to_str().unwrap().into())
}

pub fn render_link_enter(nl: &NodeLink, output: &mut WriteWithLast) -> io::Result<()> {
    output.write_all(br#"<ac:image ac:align="center""#)?;
    if !nl.title.is_empty() {
        output.write_all(format!(" ac:title=\"{}\"", nl.title).as_bytes())?;
    }
    output.write_all(b">")?;
    if nl.url.contains("://") {
        output.write_all(b"<ri:url ri:value=\"")?;
    } else {
        output.write_all(b"<ri:attachment ri:filename=\"")?;
    }

    let url = nl.url.as_bytes();
    escape_href(output, url)?;
    output.write_all(b"\"/>")?;

    Ok(())
}

pub fn render_link_leave(_nl: &NodeLink, output: &mut WriteWithLast) -> io::Result<()> {
    output.write_all(b"</ac:image>")?;
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
            "<ac:image ac:align=\"center\" ac:title=\"some title\"><ri:attachment ri:filename=\"assets/image.png\"/></ac:image>"
        );

        Ok(())
    }

    #[test]
    fn it_names_attachments_with_slashes() -> TestResult {
        let image_path = PathBuf::from("/tmp/foo/bar/assets/image.png");
        let page_path = PathBuf::from("/tmp/foo/bar");
        let image_name = attachment_name(&image_path, &page_path)?;

        assert_eq!(image_name, "assets/image.png");

        Ok(())
    }

    #[test]
    fn it_makes_absolute_path() -> TestResult {
        let image_url = String::from("./assets/image.png");
        let page_path = PathBuf::from("/tmp/foo/bar");
        let attachment = attachment_from(&image_url, &page_path);

        assert_eq!(attachment, PathBuf::from("/tmp/foo/bar/assets/image.png"));

        Ok(())
    }


}
