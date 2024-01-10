use std::collections::VecDeque;
use std::str::FromStr;

use reqwest::{Request, Response};
use serde_json::from_str;

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
        struct NextUrlIterator<'a> {
            client: &'a ConfluenceClient,
            current_page: VecDeque<responses::PageBulkWithoutBody>,
            next_url: Option<reqwest::Url>,
        }

        impl<'a> NextUrlIterator<'a> {
            fn new(
                response: reqwest::blocking::Response,
                client: &'a ConfluenceClient,
            ) -> Result<Self> {
                let current_url = response.url().clone();
                let content = response.text()?;

                let existing_page: responses::MultiEntityResult<responses::PageBulkWithoutBody> =
                    from_str(content.as_str())?;

                let next_url: Option<reqwest::Url> = existing_page
                    .links
                    .next
                    .map(|l| current_url.join(l.as_str()).unwrap()); //FIXME:
                let current_page = VecDeque::from_iter(existing_page.results.iter().cloned());
                Ok(Self {
                    current_page,
                    next_url,
                    client,
                })
            }

            fn get_next_page(&mut self) -> Result<()> {
                let response = self.client.get(self.next_url.clone().unwrap())?;
                let current_url = response.url().clone();
                let content = response.text()?;

                let existing_page: responses::MultiEntityResult<responses::PageBulkWithoutBody> =
                    from_str(content.as_str())?;

                self.next_url = existing_page
                    .links
                    .next
                    .map(|l| current_url.join(l.as_str()).unwrap()); //FIXME: error
                self.current_page = VecDeque::from_iter(existing_page.results.iter().cloned());
                Ok(())
            }
        }

        impl<'a> Iterator for NextUrlIterator<'a> {
            type Item = Result<responses::PageBulkWithoutBody>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.current_page.is_empty() && self.next_url.is_some() {
                    if let Err(err) = self.get_next_page() {
                        return Some(Err(err));
                    }
                }
                match self.current_page.pop_front() {
                    Some(i) => Some(Ok(i)),
                    None => None,
                }
            }
        }

        let response = confluence_client
            .get_all_pages_in_space(space_id)?
            .error_for_status()?;

        let results: Vec<ConfluencePage> = NextUrlIterator::new(response, confluence_client)?
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
