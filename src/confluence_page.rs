use crate::confluence_paginated::ConfluencePaginator;
use crate::{
    confluence_client::ConfluenceClient, error::ConfluenceError, markdown_page::RenderedPage,
    responses,
};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct ConfluencePage {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub content: String,
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
                    content: String::default(),
                    version: bulk_page.version.clone(),
                    parent_id: bulk_page.parent_id.clone(),
                    title: bulk_page.title.clone(),
                })
                .collect();

        let page_titles: Vec<String> = results.iter().map(|p| p.title.clone()).collect();

        println!(
            "Found {} pages already in space:\n{}",
            results.len(),
            page_titles.join("\n")
        );

        Ok(results)
    }

    pub fn get_homepage(
        confluence_client: &ConfluenceClient,
        homepage_id: &str,
    ) -> Result<ConfluencePage> {
        let existing_page: responses::PageSingle = confluence_client
            .get_page_by_id(homepage_id)?
            .error_for_status()?
            .json()?;

        let existing_content = match &existing_page.body {
            responses::BodySingle::Storage(body) => body.value.clone(),
            responses::BodySingle::AtlasDocFormat(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "atlas doc format".into(),
                }
                .into())
            }
            responses::BodySingle::View(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "view format".into(),
                }
                .into())
            }
        };
        Ok(ConfluencePage {
            id: existing_page.id,
            content: existing_content,
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
        let existing_page: responses::MultiEntityResult<responses::PageBulk> = confluence_client
            .get_page_by_title(space_id, page.title.as_str(), true)?
            .error_for_status()?
            .json()?;

        if existing_page.results.is_empty() {
            return Ok(None);
        }

        let bulk_page = &existing_page.results[0];
        let existing_content = match &bulk_page.body {
            responses::BodyBulk::Storage(body) => body.value.clone(),
            responses::BodyBulk::AtlasDocFormat(_) => {
                return Err(ConfluenceError::UnsupportedStorageFormat {
                    message: "atlas doc format".into(),
                }
                .into())
            }
            &responses::BodyBulk::Empty => todo!(),
        };
        Ok(Some(ConfluencePage {
            id: bulk_page.id.clone(),
            content: existing_content,
            version: bulk_page.version.clone(),
            parent_id: Some(bulk_page.parent_id.clone()),
            title: bulk_page.title.clone(),
        }))
    }
}
