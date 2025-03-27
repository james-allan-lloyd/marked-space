use std::collections::HashMap;

use crate::{
    confluence_client::ConfluenceClient, confluence_paginator::ConfluencePaginator,
    console::print_warning, responses,
};

fn get_user(
    client: &ConfluenceClient,
    public_name: &str,
) -> Result<Option<responses::User>, anyhow::Error> {
    let response = client.search_users(public_name)?.error_for_status()?;
    let mut results: Vec<responses::User> =
        ConfluencePaginator::<responses::SearchResult>::new(client)
            .start(response)?
            .filter_map(|f| f.ok())
            .map(|search_result_page| search_result_page.user)
            .collect();
    Ok(results.pop())
}

pub(crate) fn mention_macro(
    client: &ConfluenceClient,
    args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    let public_name = args.get("public_name").ok_or("Missing 'public_name'")?;

    let public_name_str = match public_name {
        serde_json::Value::String(s) => Ok(s),
        _ => Err(tera::Error::msg("public_name must be a string")),
    }?;

    match get_user(client, public_name_str) {
        Ok(Some(user)) => Ok(serde_json::to_value(format!(
            // trailing space prevents the tag being recognized as a markdown link
            "<ac:link ><ri:user ri:account-id=\"{}\"/></ac:link>",
            user.account_id
        ))
        .unwrap()),
        Ok(None) => {
            print_warning(&format!("Unknown user \"{}\"", public_name_str));
            Ok(serde_json::to_value("@unknown_user").unwrap())
        }

        Err(err) => Err(tera::Error::msg(err.to_string())),
    }
}

pub(crate) fn make_mention(client: ConfluenceClient) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            mention_macro(&client, args)
        },
    )
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

    fn mock_user_exists(server: &mut mockito::ServerGuard) -> mockito::Mock {
        server
            .mock("GET", "/wiki/rest/api/search/user")
            .match_query(mockito::Matcher::UrlEncoded(
                "cql".into(),
                "user.fullname~\"John Doe\"".into(),
            ))
            .with_status(201)
            .with_header("authorization", "Basic Og==")
            .with_header("content-type", "application/json")
            .with_header("X-Atlassian-Token", "no-check")
            .with_body(TEST_USER)
            .create()
    }

    #[test]
    fn it_searches_users() -> TestResult {
        // Request a new server from the pool
        let mut server = mockito::Server::new();

        // Use one of these addresses to configure your client
        let host = server.host_with_port();
        // let url = server.url();

        // Create a mock
        let mock = mock_user_exists(&mut server);
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
    fn it_prints_unknown_user_if_no_matching_user_found() -> TestResult {
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

        assert_eq!(result, "@unknown_user");

        mock.assert();
        Ok(())
    }
}
