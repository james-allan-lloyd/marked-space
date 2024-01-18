use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use crate::confluence_paginator::ConfluencePaginator;
use crate::{confluence_client::ConfluenceClient, responses};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct ConfluencePage {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub version: responses::Version,
    pub path: Option<PathBuf>,
}

impl ConfluencePage {
    pub fn version_message_prefix() -> &'static str {
        "updated by markedspace:"
    }

    fn new_from_page_bulk(bulk_page: &responses::PageBulkWithoutBody) -> Self {
        ConfluencePage {
            id: bulk_page.id.clone(),
            version: bulk_page.version.clone(),
            parent_id: bulk_page.parent_id.clone(),
            title: bulk_page.title.clone(),
            path: Self::extract_path(&bulk_page.version),
        }
    }

    pub fn extract_path(version: &responses::Version) -> Option<PathBuf> {
        if let Some(data) = version.message.strip_prefix(Self::version_message_prefix()) {
            let kvs: HashMap<&str, &str> = data
                .split(';')
                .map(|kv| {
                    let (key, value) = kv.split_once('=').unwrap();
                    (key.trim(), value.trim())
                })
                .collect();
            if let Some(path) = kvs.get("source") {
                PathBuf::from_str(path).ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_all(confluence_client: &ConfluenceClient, space_id: &str) -> Result<Vec<Self>> {
        let response = confluence_client
            .get_all_pages_in_space(space_id)?
            .error_for_status()?;

        let results: Vec<ConfluencePage> =
            ConfluencePaginator::<responses::PageBulkWithoutBody>::new(confluence_client)
                .start(response)?
                .filter_map(|f| f.ok())
                .map(|bulk_page| Self::new_from_page_bulk(&bulk_page))
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
            version: existing_page.version.clone(),
            parent_id: None,
            title: existing_page.title,
            path: Self::extract_path(&existing_page.version),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::responses;

    fn test_extract_path_from_string(s: &str) -> Option<PathBuf> {
        ConfluencePage::extract_path(&responses::Version {
            message: String::from(s),
            number: 27,
        })
    }

    fn test_extract_path_from_string_with_prefix(s: &str) -> Option<PathBuf> {
        ConfluencePage::extract_path(&responses::Version {
            message: ConfluencePage::version_message_prefix().to_owned() + s,
            number: 27,
        })
    }

    #[test]
    fn it_extracts_paths() {
        let result = test_extract_path_from_string("not a markspace update");
        assert!(result.is_none());

        let result = test_extract_path_from_string_with_prefix("checksum=CHECKSUM; source=FILE");
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path.as_os_str().to_str().unwrap(), "FILE");
    }
}
