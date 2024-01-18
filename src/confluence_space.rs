use std::path::Path;

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

    pub fn get_existing_page(
        &self,
        rendered_page: &RenderedPage,
    ) -> Result<Option<ConfluencePage>> {
        // TODO: this should be a map
        if let Some(page) = self
            .pages
            .iter()
            .find(|page| page.title == rendered_page.title)
        {
            return Ok(Some(page.clone()));
        }
        let source_marker = format!("source={}", &rendered_page.source.replace('\\', "/"));

        if let Some(page) = self
            .orphans
            .iter()
            .find(|orphan| orphan.version.message.contains(&source_marker))
        {
            Ok(Some(page.to_owned().clone()))
        } else {
            Ok(None)
        }
    }
}
