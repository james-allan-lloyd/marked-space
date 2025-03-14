use std::{io::{self, Write}, path::{Path, PathBuf}};

use comrak::nodes::NodeLink;
use regex::Regex;

use crate::confluence_storage_renderer::{escape_href, WriteWithLast};


#[derive(Debug, PartialEq)]
pub struct ImageAttachment {
    pub url: String,  // how this was specified in the markdown
    pub path: PathBuf, // the full path to the file
    pub name: String, // a simple name
}

impl ImageAttachment {
    pub fn new(url: &str, page_path: &Path) -> Self {
        let mut path = PathBuf::from(page_path);
        path.push(url);

        ImageAttachment {
            path,
            url: String::from(url),
            name: link_to_name(url)
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

        assert_eq!(attachment.path, PathBuf::from("/tmp/foo/bar/assets/image.png"));

        Ok(())
    }


}
