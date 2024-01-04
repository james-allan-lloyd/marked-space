use crate::confluence_client::ConfluenceClient;
use crate::confluence_page::ConfluencePage;
use crate::error::{ConfluenceError, Result};
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

    pub fn set_orphans(&mut self, orphans: Vec<ConfluencePage>) {
        self.orphans = orphans
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
