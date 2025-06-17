use path_clean::PathClean;
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    path::{Path, PathBuf},
};

use comrak::nodes::NodeLink;

use crate::{
    confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData},
    confluence_storage_renderer::ConfluenceStorageRenderer,
    console::print_warning,
    error::{ConfluenceError, Result},
    local_link::LocalLink,
    markdown_page::MarkdownPage,
};

#[derive(Debug)]
pub struct LinkGenerator {
    host: String,
    space_key: String,
    homepage_id: String,
    filename_to_id: HashMap<String, String>,
    filename_to_title: HashMap<String, String>,
    title_to_file: HashMap<String, String>,
    title_to_id: HashMap<String, String>,
    folders: HashSet<String>,
    page_attachment_pair_to_id: HashMap<(String, String), String>,
}

impl LinkGenerator {
    pub fn new(host: &str, space_key: &str, homepage_id: &str) -> Self {
        LinkGenerator {
            host: host.to_string(),
            space_key: space_key.to_string(),
            homepage_id: homepage_id.into(),
            filename_to_id: HashMap::default(),
            filename_to_title: HashMap::default(),
            title_to_file: HashMap::default(),
            title_to_id: HashMap::default(),
            folders: HashSet::default(),
            page_attachment_pair_to_id: HashMap::default(),
        }
    }

    #[cfg(test)]
    pub fn default_test() -> Self {
        Self::new("example.atlassian.net", "TEST", "999")
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

        if markdown_page.is_folder() {
            self.folders.insert(title.clone());
        }

        self.filename_to_title
            .insert(filename.clone(), title.clone());

        Ok(())
    }

    pub fn register_confluence_node(&mut self, confluence_node: &ConfluenceNode) {
        let title = confluence_node.title.clone();
        let id = confluence_node.id.clone();
        let homepage_id = self.homepage_id.clone();
        if let Some(filename) = self.title_to_file.get(&title) {
            self.filename_to_id.insert(filename.clone(), id.clone());
        }
        self.title_to_id.insert(title.clone(), id.clone());
        if id == homepage_id {
            self.filename_to_id
                .insert("index.md".into(), homepage_id.clone());
            self.title_to_id.insert(title.clone(), homepage_id.clone());
        } else {
            self.title_to_id.insert(title.clone(), id.clone());
            if let ConfluenceNodeType::Page(confluence_page) = &confluence_node.data {
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
        }
    }

    fn path_to_string(p: &Path) -> Result<String> {
        if let Some(s) = p.clean().to_str() {
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

    fn get_page_url(&self, filename: &Path) -> Option<String> {
        if filename == PathBuf::from("index.md") {
            return Some(self.id_to_url(&self.homepage_id));
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
        if local_link.is_page() {
            confluence_formatter.output.write_all(b"<a href=\"")?;

            let mut link_empty = true;

            if let Some(url) = self.get_page_url(&local_link.target) {
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
                    &local_link.text,
                    &confluence_formatter.source.display(),
                ));
            }

            confluence_formatter.output.write_all(b"\">")?;
            if no_children {
                confluence_formatter
                    .output
                    .write_all(self.get_file_title(&local_link.target).unwrap().as_bytes())?;
            }
        } else {
            confluence_formatter.output.write_all(b"<ac:structured-macro ac:name=\"view-file\"><ac:parameter ac:name=\"name\"><ri:attachment ri:filename=\"")?;
            confluence_formatter
                .output
                .write_all(local_link.attachment_name().as_bytes())?;
            confluence_formatter
                .output
                .write_all(b"\"/></ac:parameter></ac:structured-macro>")?;
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
        } else {
            let local_link = relative_local_link(nl, confluence_formatter);
            if local_link.is_page() {
                confluence_formatter.output.write_all(b"</a>")?;
            }
        }

        Ok(())
    }

    pub fn is_folder(&self, title: &str) -> bool {
        self.folders.contains(title)
    }

    pub fn get_nodes_to_create(&self) -> Vec<String> {
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

    pub fn is_orphaned(&self, node: &ConfluenceNode, confluence_page: &ConfluencePageData) -> bool {
        confluence_page
            .version
            .message
            .starts_with(ConfluencePageData::version_message_prefix())
            && !self.has_title(node.title.as_str())
    }

    pub fn attachment_id(&self, _relative_path: &str, _page: &MarkdownPage) -> Option<String> {
        let pair = &(_page.source.clone(), _relative_path.to_string());
        self.page_attachment_pair_to_id.get(pair).cloned()
    }

    // TODO: make the pair part of the attachment struct
    pub(crate) fn register_attachment_id(
        &mut self,
        page_source: &str,
        attachment_path: &str,
        id: &str,
    ) {
        let key = (String::from(page_source), String::from(attachment_path));
        dbg!(&key);
        let result = self
            .page_attachment_pair_to_id
            .insert(key, String::from(id));
        assert!(result.is_none(), "Should only register an attachment once")
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
        confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData},
        error::TestResult,
        responses::{self, ContentStatus, Version},
        test_helpers::markdown_page_from_str,
    };

    use super::LinkGenerator;

    #[test]
    fn it_returns_homepage_link_for_root_index_md() -> TestResult {
        let link_generator = LinkGenerator::new("example.atlassian.net", "Test", "999");

        let url_for_file = link_generator.get_page_url(&PathBuf::from("index.md"));
        assert_eq!(
            url_for_file,
            Some("https://example.atlassian.net/wiki/spaces/Test/pages/999".into())
        );
        Ok(())
    }

    #[test]
    fn it_handles_retitles() -> TestResult {
        let mut link_generator = LinkGenerator::default_test();

        let old_title = String::from("Old Title");
        let new_title = String::from("New Title");
        let source = String::from("test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;
        link_generator.register_confluence_node(&ConfluenceNode {
            id: "1".to_string(),
            title: old_title,
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: responses::Version {
                    message: String::default(),
                    number: 2,
                },
                path: Some(PathBuf::from(&source)),
                status: ContentStatus::Current,
            }),
        });

        assert!(link_generator.get_nodes_to_create().is_empty());

        let url_for_file = link_generator.get_page_url(&PathBuf::from(&source));
        assert!(url_for_file.is_some());

        Ok(())
    }

    #[test]
    fn it_handles_moves() -> TestResult {
        let mut link_generator = LinkGenerator::default_test();

        let title = String::from("Test Title");
        let old_source = String::from("old-test.md");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", title),
            &arena,
        )?)?;
        link_generator.register_confluence_node(&ConfluenceNode {
            id: "9991".to_string(),
            title,
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: responses::Version {
                    message: String::default(),
                    number: 2,
                },
                path: Some(PathBuf::from(&old_source)),
                status: ContentStatus::Current,
            }),
        });

        assert!(link_generator.get_nodes_to_create().is_empty());

        let url_for_file = link_generator.get_page_url(&PathBuf::from(&new_source));
        let expected_url =
            String::from("https://example.atlassian.net/wiki/spaces/TEST/pages/9991");
        assert_eq!(url_for_file, Some(expected_url));

        Ok(())
    }

    #[test]
    fn it_returns_none_if_title_and_file_has_changed() -> TestResult {
        let mut link_generator = LinkGenerator::default_test();

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
        link_generator.register_confluence_node(&ConfluenceNode {
            id: "9991".to_string(),
            title: old_title,
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: responses::Version {
                    message: String::default(),
                    number: 2,
                },
                path: Some(PathBuf::from(&old_source)),
                status: ContentStatus::Current,
            }),
        });

        let url_for_file = link_generator.get_page_url(&PathBuf::from(&new_source));
        assert_eq!(url_for_file, None);

        Ok(())
    }

    #[test]
    fn it_knows_which_pages_need_creating() -> TestResult {
        let mut link_generator = LinkGenerator::default_test();

        let new_title = String::from("New Title");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;

        let pages_to_create = link_generator.get_nodes_to_create();

        // let url_for_file = link_generator.get_file_url(&PathBuf::from(&new_source));
        assert_eq!(pages_to_create, vec![new_title]);

        Ok(())
    }

    #[test]
    fn it_does_not_create_homepage_because_it_always_exists() -> TestResult {
        let mut link_generator = LinkGenerator::default_test();

        let new_title = String::from("Homepage");
        let new_source = String::from("index.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("# {} \n", new_title),
            &arena,
        )?)?;

        link_generator.register_confluence_node(&ConfluenceNode {
            id: "999".to_string(),
            title: "Default Homepage".into(),
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: responses::Version {
                    message: "Default created".into(),
                    number: 1,
                },
                path: None,
                status: ContentStatus::Current,
            }),
        });

        let pages_to_create = link_generator.get_nodes_to_create();

        assert_eq!(pages_to_create, Vec::<String>::default());

        Ok(())
    }

    #[test]
    fn it_identifies_orphans() {
        let orphaned_confluence_page = ConfluenceNode {
            id: String::from("99"),
            title: String::from("Orphaned Page"),
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::from(ConfluencePageData::version_message_prefix()),
                    number: 1,
                },
                path: None, // "foo.md".to_string(),
                status: ContentStatus::Current,
            }),
        };

        let link_generator = LinkGenerator::default_test();
        assert!(link_generator.is_orphaned(
            &orphaned_confluence_page,
            orphaned_confluence_page.page_data().unwrap()
        ));
    }
}
