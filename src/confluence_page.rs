use crate::confluence_paginated::ConfluencePaginator;
use crate::{confluence_client::ConfluenceClient, markdown_page::RenderedPage, responses};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct ConfluencePage {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub version: responses::Version,
}

impl ConfluencePage {
    pub fn version_message_prefix() -> &'static str {
        "updated by markedspace:"
    }

    pub fn get_all(confluence_client: &ConfluenceClient, space_id: &str) -> Result<Vec<Self>> {
        let response = confluence_client
            .get_all_pages_in_space(space_id)?
            .error_for_status()?;

        let results: Vec<ConfluencePage> =
            ConfluencePaginator::<responses::PageBulkWithoutBody>::new(confluence_client)
                .start(response)?
                .filter_map(|f| f.ok())
                .map(|bulk_page| ConfluencePage {
                    id: bulk_page.id.clone(),
                    version: bulk_page.version.clone(),
                    parent_id: bulk_page.parent_id.clone(),
                    title: bulk_page.title.clone(),
                })
                .collect();

        Ok(results)
    }

    pub fn get_homepage(
        confluence_client: &ConfluenceClient,
        homepage_id: &str,
    ) -> Result<ConfluencePage> {
        let existing_page: responses::PageSingleWithoutBody = confluence_client
            .get_page_by_id(homepage_id)?
            .error_for_status()?
            .json()?;

        Ok(ConfluencePage {
            id: existing_page.id,
            version: existing_page.version,
            parent_id: None,
            title: existing_page.title,
        })
    }

    pub fn get_page(
        confluence_client: &ConfluenceClient,
        space_id: &str,
        page: &RenderedPage,
    ) -> Result<Option<ConfluencePage>> {
        let existing_page: responses::MultiEntityResult<responses::PageBulkWithoutBody> =
            confluence_client
                .get_page_by_title(space_id, page.title.as_str(), true)?
                .error_for_status()?
                .json()?;

        if existing_page.results.is_empty() {
            return Ok(None);
        }

        let bulk_page = &existing_page.results[0];
        Ok(Some(ConfluencePage {
            id: bulk_page.id.clone(),
            version: bulk_page.version.clone(),
            parent_id: bulk_page.parent_id.clone(),
            title: bulk_page.title.clone(),
        }))
    }
}
