use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    path::{Path, PathBuf},
};

use comrak::nodes::NodeLink;

use crate::{
    confluence_storage_renderer::ConfluenceStorageRenderer,
    error::{ConfluenceError, Result},
    local_link::LocalLink,
};

#[derive(Default)]
pub struct LinkGenerator {
    filename_to_title: HashMap<String, String>,
    titles: HashSet<String>,
}

impl LinkGenerator {
    pub fn new() -> Self {
        LinkGenerator::default()
    }

    pub fn add_file_title(&mut self, filename: &Path, title: &str) -> Result<()> {
        let title = title.to_owned();
        if self.titles.contains(&title) {
            return Err(ConfluenceError::DuplicateTitle {
                file: filename.display().to_string(),
                title,
            }
            .into());
        }
        self.titles.insert(title.clone());
        self.filename_to_title
            .insert(Self::path_to_string(filename)?, title.to_owned());
        Ok(())
    }

    fn path_to_string(p: &Path) -> Result<String> {
        if let Some(s) = p.to_str() {
            Ok(s.to_string().replace("\\", "/"))
        } else {
            Err(ConfluenceError::generic_error(
                "Failed to convert path to string",
            ))
        }
    }

    pub fn has_title(&self, title: &str) -> bool {
        self.titles.contains(&title.to_owned())
    }

    pub fn get_file_title(&self, filename: &Path) -> Option<&String> {
        if let Some(s) = Self::path_to_string(filename).ok() {
            self.filename_to_title.get(&s)
        } else {
            None
        }
    }

    pub fn enter(
        &self,
        nl: &NodeLink,
        confluence_formatter: &mut ConfluenceStorageRenderer,
    ) -> io::Result<()> {
        if nl.url.contains("://") {
            confluence_formatter.output.write_all(b"<a href=\"")?;
            confluence_formatter.output.write_all(nl.url.as_bytes())?;
            confluence_formatter.output.write_all(b"\">")?;
            return Ok(());
        }

        let local_link = relative_local_link(nl, confluence_formatter);
        if local_link.path == PathBuf::default() {
            confluence_formatter.output.write_all(b"<a href=\"#")?;

            if let Some(anchor) = local_link.anchor {
                confluence_formatter.output.write_all(anchor.as_bytes())?;
                confluence_formatter.output.write_all(b"\"")?;
            }

            confluence_formatter.output.write_all(b">")?;
        } else if let Some(confluence_title_for_file) = self.get_file_title(&local_link.path) {
            confluence_formatter.output.write_all(b"<ac:link")?;

            if let Some(anchor) = local_link.anchor {
                confluence_formatter.output.write_all(b" ac:anchor=\"")?;
                confluence_formatter.output.write_all(anchor.as_bytes())?;
                confluence_formatter.output.write_all(b"\"")?;
            }

            confluence_formatter.output.write_all(b"><ri:page")?;
            confluence_formatter
                .output
                .write_all(b" ri:content-title=\"")?;
            confluence_formatter.escape(confluence_title_for_file.as_bytes())?;
            confluence_formatter.output.write_all(b"\"/>")?;

            confluence_formatter.output.write_all(
                b"<ac:plain-text-link-body>
        <![CDATA[",
            )?;
        } else {
            println!("url: {:#?}", nl.url);
            println!("locallink: {:#?}", local_link.path);
            println!("file titles: {:#?}", self.filename_to_title);
            confluence_formatter
                .output
                .write_all(b"<!-- skipping unknown local link type -->")?;
        }

        Ok(())
    }

    pub fn exit(
        &self,
        nl: &NodeLink,
        confluence_formatter: &mut ConfluenceStorageRenderer,
    ) -> io::Result<()> {
        if nl.url.contains("://") {
            confluence_formatter.output.write_all(b"</a>")?;
            return Ok(());
        }
        let local_link = relative_local_link(nl, confluence_formatter);
        if local_link.path == PathBuf::default() {
            confluence_formatter.output.write_all(b"</a>")?;
        } else if let Some(_confluence_title_for_file) = self.get_file_title(&local_link.path) {
            confluence_formatter
                .output
                .write_all(b"]]></ac:plain-text-link-body></ac:link>")?;
        }

        Ok(())
    }
}

fn relative_local_link(
    nl: &NodeLink,
    confluence_formatter: &mut ConfluenceStorageRenderer<'_>,
) -> LocalLink {
    LocalLink::from_str(&nl.url, &confluence_formatter.source.parent().unwrap()).unwrap()
}
