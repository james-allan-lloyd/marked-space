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
        })
    }

    pub fn find_orphaned_pages(
        &mut self,
        confluence_client: &ConfluenceClient,
        link_generator: &mut LinkGenerator,
        space_dir: &Path,
    ) -> Result<()> {
        let orphaned_pages: Vec<ConfluencePage> =
            ConfluencePage::get_all(confluence_client, &self.id)?
                .into_iter()
                .filter_map(|confluence_page| {
                    link_generator
                        .add_title_id(&confluence_page.title, &confluence_page.id)
                        .unwrap();
                    if confluence_page
                        .version
                        .message
                        .starts_with(ConfluencePage::version_message_prefix())
                        && !link_generator.has_title(confluence_page.title.as_str())
                    {
                        Some(confluence_page)
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
        confluence_client: &ConfluenceClient,
        page: &RenderedPage,
    ) -> Result<Option<ConfluencePage>> {
        let result = ConfluencePage::get_page(confluence_client, &self.id, page)?;
        if result.is_some() {
            return Ok(result);
        }
        let source_marker = format!("source={}", &page.source.replace('\\', "/"));
        Ok(self
            .orphans
            .iter()
            .find(|orphan| orphan.version.message.contains(&source_marker))
            .cloned())
    }
}
