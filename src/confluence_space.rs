use std::path::{Path, PathBuf};

use crate::confluence_client::ConfluenceClient;
use crate::confluence_page::ConfluencePage;
use crate::error::{ConfluenceError, Result};
use crate::link_generator::LinkGenerator;
use crate::markdown_page::RenderedPage;
use crate::responses;

#[derive(Debug)]
pub struct ConfluenceSpace {
    pub id: String,
    pub homepage_id: String,
    orphans: Vec<ConfluencePage>,
    pages: Vec<ConfluencePage>,
}

impl ConfluenceSpace {
    #[cfg(test)]
    pub fn new(id: &str) -> ConfluenceSpace {
        ConfluenceSpace {
            id: id.to_string(),
            homepage_id: "99".to_string(),
            orphans: Vec::default(),
            pages: Vec::default(),
        }
    }

    pub fn get(confluence_client: &ConfluenceClient, space_key: &str) -> Result<ConfluenceSpace> {
        let resp = match confluence_client.get_space_by_key(space_key) {
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
                space_key
            )));
        }

        let parsed_space = serde_json::from_value::<responses::Space>(json["results"][0].clone())?;

        Ok(ConfluenceSpace {
            id: parsed_space.id,
            homepage_id: parsed_space.homepage_id,
            orphans: Vec::default(),
            pages: Vec::default(),
        })
    }

    pub fn read_all_pages(&mut self, confluence_client: &ConfluenceClient) -> Result<()> {
        self.pages = ConfluencePage::get_all(confluence_client, &self.id)?;
        Ok(())
    }

    pub fn find_orphaned_pages(
        &mut self,
        link_generator: &mut LinkGenerator,
        space_dir: &Path,
    ) -> Result<()> {
        link_generator.homepage_id = Some(self.homepage_id.clone());
        let orphaned_pages: Vec<ConfluencePage> = self
            .pages
            .iter()
            .filter_map(|confluence_page| {
                link_generator.register_confluence_page(&confluence_page);
                if confluence_page
                    .version
                    .message
                    .starts_with(ConfluencePage::version_message_prefix())
                    && !link_generator.has_title(confluence_page.title.as_str())
                {
                    Some(confluence_page.clone())
                } else {
                    None
                }
            })
            .collect();
        orphaned_pages.iter().for_each(|p| {
            if let Some(path) = &p.path {
                if !space_dir.join(path).exists() {
                    println!(
                        "Orphaned page detected \"{}\" (probably deleted), version comment: {}",
                        p.title, p.version.message
                    );
                }
            } else {
                println!(
                    "Orphaned page detected \"{}\" (probably created outside of markedspace), version comment: {}",
                    p.title, p.version.message
                );
            }
        });
        self.orphans = orphaned_pages;
        Ok(())
    }

    pub fn get_existing_page(&self, rendered_page: &RenderedPage) -> Option<ConfluencePage> {
        // TODO: this should be a map
        let filename = rendered_page.source.replace('\\', "/");
        if filename == "index.md" {
            return self
                .pages
                .iter()
                .find(|page| page.id == self.homepage_id)
                .cloned();
        }
        if let Some(page) = self.pages.iter().find(|page| {
            page.title == rendered_page.title || page.path == Some(PathBuf::from(&filename))
        }) {
            return Some(page.clone());
        }
        if let Some(page) = self
            .orphans
            .iter()
            .find(|orphan| orphan.path == Some(PathBuf::from(&filename)))
        {
            Some(page.to_owned().clone())
        } else {
            None
        }
    }

    pub fn add_page(&mut self, from: ConfluencePage) {
        self.pages.push(from);
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::{confluence_page::ConfluencePage, markdown_page::RenderedPage, responses};

    use super::ConfluenceSpace;

    #[test]
    fn it_finds_retitled_files() {
        let mut space = ConfluenceSpace::new("TEST");

        space.add_page(ConfluencePage {
            id: "1".to_string(),
            title: "Old Title".to_string(),
            parent_id: None,
            version: responses::Version {
                message: String::default(),
                number: 2,
            },
            path: Some(PathBuf::from("test.md")),
        });

        let existing_page = space.get_existing_page(&RenderedPage {
            title: "New Title".to_string(),
            content: String::default(),
            source: "test.md".to_string(),
            parent: None,
            checksum: String::default(),
        });

        assert!(existing_page.is_some())
    }

    #[test]
    fn it_returns_homepage_for_root_index_md() {
        let mut space = ConfluenceSpace::new("TEST");

        space.add_page(ConfluencePage {
            id: space.homepage_id.clone(),
            title: "Existing Home Page".to_string(),
            parent_id: None,
            version: responses::Version {
                message: String::default(),
                number: 2,
            },
            // path: Some(PathBuf::from("test.md")),
            path: None,
        });

        let existing_page = space.get_existing_page(&RenderedPage {
            title: "New Title".to_string(),
            content: String::default(),
            source: "index.md".to_string(),
            parent: None,
            checksum: String::default(),
        });

        assert!(existing_page.is_some());
        let existing_page = existing_page.unwrap();
        assert_eq!(existing_page.id, space.homepage_id);
    }
}
