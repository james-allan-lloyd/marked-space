use std::{
    collections::HashMap,
    io::{self, Write},
    path::{Path, PathBuf},
};

use comrak::nodes::NodeLink;

use crate::{
    confluence_page::ConfluencePage,
    confluence_storage_renderer::ConfluenceStorageRenderer,
    console::print_warning,
    error::{ConfluenceError, Result},
    local_link::LocalLink,
    markdown_page::MarkdownPage,
};

#[derive(Default, Debug)]
pub struct LinkGenerator {
    host: String,
    space_key: String,
    pub homepage_id: Option<String>,
    filename_to_id: HashMap<String, String>,
    filename_to_title: HashMap<String, String>,
    title_to_file: HashMap<String, String>,
    title_to_id: HashMap<String, String>,
}

impl LinkGenerator {
    pub fn new(host: &str, space_key: &str) -> Self {
        LinkGenerator {
            host: host.to_string(),
            space_key: space_key.to_string(),
            homepage_id: None,
            filename_to_id: HashMap::default(),
            filename_to_title: HashMap::default(),
            title_to_file: HashMap::default(),
            title_to_id: HashMap::default(),
        }
    }

    pub fn register_markdown_page(&mut self, markdown_page: &MarkdownPage) -> Result<()> {
        let title = markdown_page.title.to_owned();
        let filename = markdown_page.source.replace('\\', "/");
        if self.title_to_file.contains_key(&title) {
            return Err(ConfluenceError::DuplicateTitle {
                file: filename,
                title,
            }
            .into());
        }
        self.title_to_file.insert(title.clone(), filename.clone());

        self.filename_to_title
            .insert(filename.clone(), title.clone());
        Ok(())
    }

    pub fn register_confluence_page(&mut self, confluence_page: &ConfluencePage) {
        let title = confluence_page.title.clone();
        let id = confluence_page.id.clone();
        if let Some(filename) = self.title_to_file.get(&title) {
            self.filename_to_id.insert(filename.clone(), id.clone());
        }
        self.title_to_id.insert(title, id.clone());
        if let Some(version_path) = &confluence_page.path {
            if let Ok(p) = Self::path_to_string(version_path) {
                // this is the move logic: use the path from the version string if there isn't already a mapping to the id through title.
                self.filename_to_id.entry(p).or_insert(id);
            } else {
                print_warning(&format!(
                    "failed to convert {} to string",
                    version_path.display()
                ));
            }
        }
    }

    fn path_to_string(p: &Path) -> Result<String> {
        if let Some(s) = p.to_str() {
            Ok(s.to_string().replace('\\', "/"))
        } else {
            Err(ConfluenceError::generic_error(
                "Failed to convert path to string",
            ))
        }
    }

    pub fn has_title(&self, title: &str) -> bool {
        self.title_to_file.contains_key(title)
    }

    pub fn get_file_id(&self, filename: &Path) -> Option<String> {
        Self::path_to_string(filename)
            .ok()
            .and_then(|s| self.filename_to_id.get(&s))
            .cloned()
    }

    fn id_to_url(&self, id: &str) -> String {
        format!(
            "https://{}/wiki/spaces/{}/pages/{}",
            self.host, self.space_key, id
        )
    }

    fn get_file_url(&self, filename: &Path) -> Option<String> {
        if filename == PathBuf::from("index.md") {
            // return Some((unwrap().as_ref()));
            return self.homepage_id.clone().map(|id| self.id_to_url(&id));
        }
        if let Ok(s) = Self::path_to_string(filename) {
            self.filename_to_id.get(&s).map(|id| self.id_to_url(id))
            // .or_else(|| self.version_path_to_id.get(&s).map(|id| self.id_to_url(id)))
        } else {
            None
        }
    }

    fn get_file_title(&self, path: &Path) -> Option<String> {
        let s = Self::path_to_string(path).unwrap();
        self.filename_to_title.get(&s).cloned()
    }

    pub fn enter(
        &self,
        nl: &NodeLink,
        confluence_formatter: &mut ConfluenceStorageRenderer,
        no_children: bool,
    ) -> io::Result<()> {
        if nl.url.contains("://") {
            confluence_formatter.output.write_all(b"<a href=\"")?;
            confluence_formatter.output.write_all(nl.url.as_bytes())?;
            confluence_formatter.output.write_all(b"\">")?;
            return Ok(());
        }

        let local_link = relative_local_link(nl, confluence_formatter);
        confluence_formatter.output.write_all(b"<a href=\"")?;

        let mut link_empty = true;

        if let Some(url) = self.get_file_url(&local_link.path) {
            link_empty = false;
            confluence_formatter.output.write_all(url.as_bytes())?;
        }

        if let Some(anchor) = local_link.anchor {
            link_empty = false;
            confluence_formatter.output.write_all(b"#")?;
            confluence_formatter.output.write_all(anchor.as_bytes())?;
        }

        if link_empty {
            print_warning(&format!(
                "file link {} in {} couldn't be resolved",
                &local_link.path.display(),
                &confluence_formatter.source.display(),
            ));
        }

        confluence_formatter.output.write_all(b"\">")?;

        if no_children {
            confluence_formatter
                .output
                .write_all(self.get_file_title(&local_link.path).unwrap().as_bytes())?;
        }

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

    pub fn get_pages_to_create(&self) -> Vec<String> {
        self.title_to_file
            .iter()
            .filter_map(|(title, file)| {
                if !self.filename_to_id.contains_key(file) {
                    Some(title.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

fn relative_local_link(
    nl: &NodeLink,
    confluence_formatter: &mut ConfluenceStorageRenderer<'_>,
) -> LocalLink {
    LocalLink::from_str(&nl.url, confluence_formatter.source.parent().unwrap()).unwrap()
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::{
        confluence_page::ConfluencePage, error::TestResult, markdown_page::MarkdownPage, responses,
        template_renderer::TemplateRenderer,
    };

    use super::LinkGenerator;

    fn markdown_page_from_str<'a>(
        filename: &str,
        content: &str,
        arena: &'a Arena<AstNode<'a>>,
    ) -> crate::error::Result<MarkdownPage<'a>> {
        MarkdownPage::from_str(
            &PathBuf::from(filename),
            content,
            arena,
            filename.to_string(),
            &mut TemplateRenderer::default()?,
        )
    }

    #[test]
    fn it_returns_homepage_link_for_root_index_md() -> TestResult {
        let mut link_generator = LinkGenerator::new("example.atlassian.net", "Test");
        link_generator.homepage_id = Some("999".to_string());

        let url_for_file = link_generator.get_file_url(&PathBuf::from("index.md"));
        assert_eq!(
            url_for_file,
            Some("https://example.atlassian.net/wiki/spaces/Test/pages/999".into())
        );
        Ok(())
    }

    #[test]
    fn it_handles_retitles() -> TestResult {
        let mut link_generator = LinkGenerator::default();

        let old_title = String::from("Old Title");
        let new_title = String::from("New Title");
        let source = String::from("test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;
        link_generator.register_confluence_page(&ConfluencePage {
            id: "1".to_string(),
            title: old_title,
            parent_id: None,
            version: responses::Version {
                message: String::default(),
                number: 2,
            },
            path: Some(PathBuf::from(&source)),
        });

        assert!(link_generator.get_pages_to_create().is_empty());

        let url_for_file = link_generator.get_file_url(&PathBuf::from(&source));
        assert!(url_for_file.is_some());

        Ok(())
    }

    #[test]
    fn it_handles_moves() -> TestResult {
        let mut link_generator = LinkGenerator::new("example.atlassian.net", "TEST");

        let title = String::from("Test Title");
        let old_source = String::from("old-test.md");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", title),
            &arena,
        )?)?;
        link_generator.register_confluence_page(&ConfluencePage {
            id: "9991".to_string(),
            title,
            parent_id: None,
            version: responses::Version {
                message: String::default(),
                number: 2,
            },
            path: Some(PathBuf::from(&old_source)),
        });

        assert!(link_generator.get_pages_to_create().is_empty());

        let url_for_file = link_generator.get_file_url(&PathBuf::from(&new_source));
        let expected_url =
            String::from("https://example.atlassian.net/wiki/spaces/TEST/pages/9991");
        assert_eq!(url_for_file, Some(expected_url));

        Ok(())
    }

    #[test]
    fn it_returns_none_if_title_and_file_has_changed() -> TestResult {
        let mut link_generator = LinkGenerator::new("example.atlassian.net", "TEST");

        let old_title = String::from("Old Title");
        let new_title = String::from("New Title");
        let old_source = String::from("old-test.md");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;
        link_generator.register_confluence_page(&ConfluencePage {
            id: "9991".to_string(),
            title: old_title,
            parent_id: None,
            version: responses::Version {
                message: String::default(),
                number: 2,
            },
            path: Some(PathBuf::from(&old_source)),
        });

        let url_for_file = link_generator.get_file_url(&PathBuf::from(&new_source));
        assert_eq!(url_for_file, None);

        Ok(())
    }

    #[test]
    fn it_knows_which_pages_need_creating() -> TestResult {
        let mut link_generator = LinkGenerator::new("example.atlassian.net", "TEST");

        let new_title = String::from("New Title");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;

        let pages_to_create = link_generator.get_pages_to_create();

        // let url_for_file = link_generator.get_file_url(&PathBuf::from(&new_source));
        assert_eq!(pages_to_create, vec![new_title]);

        Ok(())
    }
}
