use std::collections::VecDeque;

use serde_json::from_str;

use crate::confluence_client::ConfluenceClient;
use crate::error::Result;
use crate::responses;

pub struct ConfluencePaginator<'a, T> {
    client: &'a ConfluenceClient,
    current_page: VecDeque<T>,
    next_url: Option<reqwest::Url>,
}

impl<'a, T> ConfluencePaginator<'a, T>
where
    T: serde::de::DeserializeOwned + Clone + std::fmt::Debug,
{
    pub fn new(client: &'a ConfluenceClient) -> Self {
        Self {
            client,
            current_page: VecDeque::default(),
            next_url: None,
        }
    }

    pub fn start(&mut self, response: reqwest::blocking::Response) -> Result<&mut Self> {
        self.parse_response(response.error_for_status()?)
    }

    fn parse_response(&mut self, response: reqwest::blocking::Response) -> Result<&mut Self> {
        let current_url = response.url().clone();
        let content = response.text()?;

        let existing_page: responses::MultiEntityResult<T> = from_str(content.as_str())?;

        self.next_url = existing_page
            .links
            .and_then(|l| l.next)
            .and_then(|n| current_url.join(&n).ok());
        self.current_page = VecDeque::from_iter(existing_page.results.iter().cloned());
        let links = existing_page.links.unwrap();
        self.next_url = links.next.map(|l| current_url.join(l.as_str()).unwrap()); //FIXME: error
        Ok(self)
    }

    fn get_next_page(&mut self) -> Result<&mut Self> {
        if let Some(next_url) = &self.next_url {
            let response = self.client.get(next_url)?.error_for_status()?;
            self.parse_response(response)
        } else {
            Err(anyhow::anyhow!("No next url!"))
        }
    }
}

impl<T> Iterator for ConfluencePaginator<'_, T>
where
    T: serde::de::DeserializeOwned + Clone + std::fmt::Debug,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_page.is_empty() && self.next_url.is_some() {
            if let Err(err) = self.get_next_page() {
                return Some(Err(err));
            }
        }
        self.current_page.pop_front().map(Ok)
    }
}

#[cfg(test)]
mod test {
    use mockito::Matcher;
    use serde_json::json;

    use crate::{
        confluence_client,
        error::TestResult,
        responses::{self, Descendant},
    };

    use super::ConfluencePaginator;

    #[test]
    fn it_paginates_pages() -> TestResult {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();
        let client = confluence_client::ConfluenceClient::new_insecure(&host);

        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/wiki/api/v2/pages/\d+/descendants$".to_string()),
            )
            .with_status(200)
            .match_query(Matcher::Any)
            .with_body(
                json!({
                    "results": [
                        {
                            "id": "103622644",
                            "status": "current",
                            "title": "Our vision, mision, strategy",
                            "parentId": "103612418",
                            "depth": 0,
                            "childPosition": 0,
                            "type": "page"
                        },
                    ],
                    "_links": {}
                })
                .to_string()
                .as_bytes(),
            )
            .create();

        let response = client.get_all_pages_from_homepage("1")?;

        let mut descendants_iter: ConfluencePaginator<responses::Descendant> =
            ConfluencePaginator::new(&client);
        let descendants = descendants_iter
            .start(response)?
            .filter_map(|d| d.ok())
            .collect::<Vec<Descendant>>();

        assert!(!descendants.is_empty());

        mock.assert();

        Ok(())
    }

    #[test]
    fn it_calls_cursor() -> TestResult {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();
        let client = confluence_client::ConfluenceClient::new_insecure(&host);

        let cursor_value = String::from("cursor1234");

        let mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/wiki/api/v2/pages/\d+/descendants$".to_string()),
            )
            .with_status(200)
            .match_query(Matcher::Any)
            .with_body(
                json!({
                    "results": [
                        {
                            "id": "103622644",
                            "status": "current",
                            "title": "Our vision, mision, strategy",
                            "parentId": "103612418",
                            "depth": 1,
                            "childPosition": 0,
                            "type": "page"
                        },
                    ],
                    "_links": {
                        "next": format!("/wiki/api/v2/pages/1/descendants?cursor={}", cursor_value)
                    }
                })
                .to_string()
                .as_bytes(),
            )
            .create();

        let cursor_mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/wiki/api/v2/pages/\d+/descendants$".to_string()),
            )
            .match_query(Matcher::UrlEncoded("cursor".into(), cursor_value))
            .with_status(200)
            .with_body(
                json!({
                    "results": [
                        {
                            "id": "103622644",
                            "status": "current",
                            "title": "Our vision, mision, strategy",
                            "parentId": "103612418",
                            "depth": 0,
                            "childPosition": 0,
                            "type": "page"
                        },
                    ],
                    "_links": {}
                })
                .to_string()
                .as_bytes(),
            )
            .create();

        let response = client.get_all_pages_from_homepage("1")?;

        let mut descendants_iter: ConfluencePaginator<responses::Descendant> =
            ConfluencePaginator::new(&client);
        let descendants = descendants_iter
            .start(response)?
            .filter_map(|d| d.ok())
            .inspect(|d| println!("{}", d.title))
            .collect::<Vec<Descendant>>();

        assert!(!descendants.is_empty());

        mock.assert();
        cursor_mock.assert();

        Ok(())
    }
}
