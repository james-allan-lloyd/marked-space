use std::{collections::HashMap, sync::RwLock};

use crate::{
    confluence_client::ConfluenceClient, confluence_paginator::ConfluencePaginator,
    console::print_warning, error::Result, responses,
};

fn get_user(client: &ConfluenceClient, public_name: &str) -> Result<Option<responses::User>> {
    let response = client.search_users(public_name)?.error_for_status()?;
    let mut results: Vec<responses::User> =
        ConfluencePaginator::<responses::SearchResult>::new(client)
            .start(response)?
            .filter_map(|f| f.ok())
            .map(|search_result_page| search_result_page.user)
            .collect();
    Ok(results.pop())
}

pub struct CachedMentions {
    client: ConfluenceClient,
    cache: RwLock<HashMap<String, Option<String>>>,
}

impl CachedMentions {
    pub fn new(client: ConfluenceClient) -> CachedMentions {
        Self {
            client,
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn format_as_user_link(&self, account_id: &str) -> tera::Value {
        serde_json::to_value(format!(
            // trailing space prevents the tag being recognized as a markdown link
            "<ac:link ><ri:user ri:account-id=\"{}\"/></ac:link>",
            account_id
        ))
        .unwrap()
    }

    fn read_cache(&self, public_name: &str) -> Option<Option<String>> {
        self.cache
            .read()
            .unwrap()
            .get(public_name)
            .map(|optional_account_id| optional_account_id.to_owned())
    }

    fn account_id(&self, public_name: &str) -> tera::Result<Option<String>> {
        if let Some(optional_account_id) = self.read_cache(public_name) {
            Ok(optional_account_id.to_owned())
        } else {
            let mut write_cache = self.cache.write().unwrap();
            match get_user(&self.client, public_name) {
                Ok(Some(user)) => {
                    write_cache.insert(public_name.to_owned(), Some(user.account_id.clone()));
                    Ok(Some(user.account_id))
                }
                Ok(None) => {
                    write_cache.insert(String::from(public_name), None);
                    print_warning(&format!("Unknown user \"{}\"", public_name));
                    Ok(None)
                }

                Err(err) => Err(tera::Error::msg(err.to_string())),
            }
        }
    }
}

impl tera::Function for CachedMentions {
    fn call(&self, args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
        let public_name = args.get("public_name").ok_or("Missing 'public_name'")?;

        let public_name_str = match public_name {
            serde_json::Value::String(s) => Ok(s),
            _ => Err(tera::Error::msg("public_name must be a string")),
        }?;

        match self.account_id(public_name_str)? {
            Some(account_id) => Ok(self.format_as_user_link(&account_id)),
            None => Ok(public_name.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        confluence_client, error::TestResult, frontmatter::FrontMatter,
        template_renderer::TemplateRenderer,
    };

    static NO_USERS: &str = r#"{"results":[],"start":0,"limit":25,"size":0,"totalSize":0,"cqlQuery":"user.fullname ~ \"dave\"","searchDuration":76,"_links":{"base":"https://jimjim256.atlassian.net/wiki","context":"/wiki"}}"#;

    static TEST_USER: &str = r###"
{
  "results": [
    {
      "user": {
        "type": "known",
        "accountId": "some-atlassian-uuid",
        "accountType": "atlassian",
        "email": "john.doe@example.com",
        "publicName": "John Doe",
        "profilePicture": {
          "path": "/wiki/aa-avatar/some-atlassian-uuid",
          "width": 48,
          "height": 48,
          "isDefault": false
        },
        "displayName": "John Doe",
        "isExternalCollaborator": false,
        "_expandable": {
          "operations": "",
          "personalSpace": ""
        },
        "_links": {
          "self": "http://example.atlassian.net/wiki/rest/api/user?accountId=some-atlassian-uuid"
        }
      },
      "title": "John Doe",
      "excerpt": "",
      "url": "/people/some-atlassian-uuid",
      "breadcrumbs": [],
      "entityType": "user",
      "iconCssClass": "aui-icon content-type-profile",
      "lastModified": "2025-03-22T08:22:14.998Z",
      "score": 0
    }
  ],
  "start": 0,
  "limit": 25,
  "size": 1,
  "totalSize": 1,
  "cqlQuery": "user.publicName = John Doe",
  "searchDuration": 103,
  "_links": {
    "base": "https://example.atlassian.net/wiki",
    "context": "/wiki"
  }
}
"###;

    fn mock_user_search(
        server: &mut mockito::ServerGuard,
        user_name: &str,
        response: &str,
    ) -> mockito::Mock {
        server
            .mock("GET", "/wiki/rest/api/search/user")
            .match_query(mockito::Matcher::UrlEncoded(
                "cql".into(),
                format!("user.fullname~\"{}\"", user_name),
            ))
            .with_status(200)
            .with_header("authorization", "Basic Og==")
            .with_header("content-type", "application/json")
            .with_header("X-Atlassian-Token", "no-check")
            .with_body(response)
            .expect(1) // only called once
            .create()
    }

    #[test]
    fn it_searches_users() -> TestResult {
        let mut server = mockito::Server::new();
        let host = server.host_with_port();

        let mock = mock_user_search(&mut server, "John Doe", TEST_USER);
        let client = confluence_client::ConfluenceClient::new_insecure(&host);

        let mut template_renderer = TemplateRenderer::default_with_client(&client)?;

        let result = template_renderer.render_template_str(
            "test.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        mock.assert();

        assert_eq!(
            result,
            // trailing space on the ac:link stops it from being seen as a markdown link
            "<ac:link ><ri:user ri:account-id=\"some-atlassian-uuid\"/></ac:link>"
        );

        Ok(())
    }

    #[test]
    fn it_errors_if_public_name_not_a_string() -> TestResult {
        let server = mockito::Server::new();
        let client = confluence_client::ConfluenceClient::new_insecure(&server.host_with_port());

        let mut template_renderer = TemplateRenderer::default_with_client(&client)?;

        let result = template_renderer.render_template_str(
            "test.md",
            "{{ mention(public_name=7) }}",
            &FrontMatter::default(),
        );

        assert!(result.is_err());
        assert!(format!("{:?}", result.err().unwrap()).contains("public_name must be a string"));

        Ok(())
    }

    #[test]
    fn it_prints_mention_name_if_no_matching_user_found() -> TestResult {
        let mut server = mockito::Server::new();
        let client = confluence_client::ConfluenceClient::new_insecure(&server.host_with_port());
        let mut template_renderer = TemplateRenderer::default_with_client(&client)?;

        let mock = server
            .mock("GET", "/wiki/rest/api/search/user")
            .match_query(mockito::Matcher::UrlEncoded(
                "cql".into(),
                "user.fullname~\"John Doe\"".into(),
            ))
            .with_status(200)
            .with_header("authorization", "Basic Og==")
            .with_header("content-type", "application/json")
            .with_header("X-Atlassian-Token", "no-check")
            .with_body(NO_USERS)
            .create();

        let result = template_renderer.render_template_str(
            "test.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        assert_eq!(result, "John Doe");

        mock.assert();
        Ok(())
    }

    #[test]
    fn it_caches_account_ids() -> TestResult {
        let mut server = mockito::Server::new();
        let client = confluence_client::ConfluenceClient::new_insecure(&server.host_with_port());
        let mut template_renderer = TemplateRenderer::default_with_client(&client)?;

        let mock = mock_user_search(&mut server, "John Doe", TEST_USER);

        template_renderer.render_template_str(
            "test.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        template_renderer.render_template_str(
            "test2.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn it_caches_unknown_accounts_too() -> TestResult {
        let mut server = mockito::Server::new();
        let client = confluence_client::ConfluenceClient::new_insecure(&server.host_with_port());
        let mut template_renderer = TemplateRenderer::default_with_client(&client)?;

        let mock = mock_user_search(&mut server, "John Doe", NO_USERS);

        template_renderer.render_template_str(
            "test.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        template_renderer.render_template_str(
            "test2.md",
            "{{ mention(public_name=\"John Doe\") }}",
            &FrontMatter::default(),
        )?;

        mock.assert();
        Ok(())
    }
}
