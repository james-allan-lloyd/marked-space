use std::path::Path;

use serde_json::json;

use crate::confluence_client::ConfluenceClient;
use crate::confluence_page::ConfluencePage;
use crate::error::{ConfluenceError, Result};
use crate::link_generator::LinkGenerator;

use crate::responses::{self, PageSingleWithoutBody, Version};
use crate::sync_operation::{SyncOperation, SyncOperationResult};

#[derive(Debug)]
pub struct ConfluenceSpace {
    pub id: String,
    pub homepage_id: String,
    pages: Vec<ConfluencePage>,
}

impl ConfluenceSpace {
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
        Ok(())
    }

    pub fn get_existing_page(&self, page_id: &str) -> Option<ConfluencePage> {
        self.pages.iter().find(|page| page.id == page_id).cloned()
    }

    pub fn add_page(&mut self, from: ConfluencePage) {
        self.pages.push(from);
    }

    pub fn create_initial_pages(
        &mut self,
        link_generator: &mut LinkGenerator,
        confluence_client: &ConfluenceClient,
    ) -> Result<()> {
        Ok(for title in link_generator.get_pages_to_create() {
            let op = SyncOperation::start(format!("Creating new page \"{}\"", title), true);
            // it's important that we have a version message to make move detection
            // work, but you can't set the version string for a create call, so we
            // create a page with empty content, then update it with the new stuff.
            // Means we'll always have at least two versions.
            let resp = confluence_client.create_page(json!({
                "spaceId": self.id,
                "status": "current",
                "title": title,
                "parentId": self.homepage_id.clone(),
            }))?;
            if !resp.status().is_success() {
                op.end(SyncOperationResult::Error);
                return Err(ConfluenceError::failed_request(resp));
            }

            let page: PageSingleWithoutBody = resp.json()?;
            let existing_page = ConfluencePage {
                id: page.id,
                title: title.clone(),
                parent_id: Some(self.homepage_id.clone()),
                version: Version {
                    number: 1,
                    message: String::default(),
                },
                path: None,
            };
            link_generator.register_confluence_page(&existing_page);
            self.add_page(existing_page);
            op.end(SyncOperationResult::Created);
        })
    }
}
