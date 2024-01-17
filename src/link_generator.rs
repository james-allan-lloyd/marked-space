use std::{
    collections::HashMap,
    io::{self, Write},
    path::Path,
};

use comrak::nodes::NodeLink;

use crate::{
    confluence_storage_renderer::ConfluenceStorageRenderer,
    error::{ConfluenceError, Result},
    local_link::LocalLink,
};

#[derive(Default)]
pub struct LinkGenerator {
    host: String,
    space_key: String,
    filename_to_id: HashMap<String, String>,
    title_to_file: HashMap<String, String>,
    title_to_id: HashMap<String, String>,
}

impl LinkGenerator {
    pub fn new(host: &str, space_key: &str) -> Self {
        LinkGenerator {
            host: host.to_string(),
            space_key: space_key.to_string(),
            filename_to_id: HashMap::default(),
            title_to_file: HashMap::default(),
            title_to_id: HashMap::default(),
        }
    }

    pub fn add_file_title(&mut self, filename: &Path, title: &str) -> Result<()> {
        let title = title.to_owned();
        if self.title_to_file.contains_key(&title) {
            return Err(ConfluenceError::DuplicateTitle {
                file: filename.display().to_string(),
                title,
            }
            .into());
        }
        self.title_to_file
            .insert(title.clone(), Self::path_to_string(filename)?);
        Ok(())
    }

    pub fn add_title_id(&mut self, title: &str, id: &str) -> Result<()> {
        self.title_to_id.insert(title.to_string(), id.to_string());

        if let Some(filename) = self.title_to_file.get(title) {
            self.filename_to_id.insert(filename.clone(), id.to_string());
        }
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
        self.title_to_file.contains_key(&title.to_owned())
    }

    pub fn get_file_id(&self, filename: &std::path::PathBuf) -> Option<String> {
        Self::path_to_string(filename)
            .ok()
            .and_then(|s| self.filename_to_id.get(&s))
            .cloned()
    }

    pub fn get_file_url(&self, filename: &Path) -> Option<String> {
        if let Some(s) = Self::path_to_string(filename).ok() {
            if let Some(id) = self.filename_to_id.get(&s) {
                Some(format!(
                    "https://{}/wiki/spaces/{}/pages/{}",
                    self.host, self.space_key, id
                ))
            } else {
                None
            }
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
        confluence_formatter.output.write_all(b"<a href=\"")?;

        if let Some(url) = self.get_file_url(&local_link.path) {
            confluence_formatter.output.write_all(url.as_bytes())?;
        }

        if let Some(anchor) = local_link.anchor {
            confluence_formatter.output.write_all(b"#")?;
            confluence_formatter.output.write_all(anchor.as_bytes())?;
        }

        confluence_formatter.output.write_all(b"\">")?;

        Ok(())
    }

    pub fn exit(
        &self,
        _nl: &NodeLink,
        confluence_formatter: &mut ConfluenceStorageRenderer,
    ) -> io::Result<()> {
        confluence_formatter.output.write_all(b"</a>")?;

        Ok(())
    }
}

fn relative_local_link(
    nl: &NodeLink,
    confluence_formatter: &mut ConfluenceStorageRenderer<'_>,
) -> LocalLink {
    LocalLink::from_str(&nl.url, &confluence_formatter.source.parent().unwrap()).unwrap()
}
