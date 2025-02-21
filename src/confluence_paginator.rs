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
    T: serde::de::DeserializeOwned + Clone,
{
    pub fn new(client: &'a ConfluenceClient) -> Self {
        Self {
            client,
            current_page: VecDeque::default(),
            next_url: None,
        }
    }

    pub fn start(&mut self, response: reqwest::blocking::Response) -> Result<&mut Self> {
        self.parse_response(response)
    }

    fn parse_response(&mut self, response: reqwest::blocking::Response) -> Result<&mut Self> {
        let current_url = response.url().clone();
        let content = response.text()?;

        let existing_page: responses::MultiEntityResult<T> = from_str(content.as_str())?;

        self.next_url = existing_page
            .links
            .unwrap()
            .next
            .map(|l| current_url.join(l.as_str()).unwrap()); //FIXME: error
        self.current_page = VecDeque::from_iter(existing_page.results.iter().cloned());
        Ok(self)
    }

    fn get_next_page(&mut self) -> Result<&mut Self> {
        let response = self
            .client
            .get(self.next_url.clone().unwrap())?
            .error_for_status()?;
        self.parse_response(response)
    }
}

impl<T> Iterator for ConfluencePaginator<'_, T>
where
    T: serde::de::DeserializeOwned + Clone,
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
