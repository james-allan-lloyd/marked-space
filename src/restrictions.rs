use serde_json::json;

use crate::{confluence_client::ConfluenceClient, confluence_page::ConfluencePage};

pub enum RestrictionType<'a> {
    SingleEditor(&'a serde_json::Value), // only the current user can edit
    OpenSpace,                           // anyone in the space can edit
}

fn restriction_body(editor_list: &serde_json::Value) -> serde_json::Value {
    json!({
        "results": [
            {
                "operation": "read",
                "restrictions": {
                    "user": {
                        "results": [],
                        "start": 0,
                        "limit": 100,
                        "size": 0
                    },
                    "group": {
                        "results": [],
                        "start": 0,
                        "limit": 100,
                        "size": 0
                    }
                },
            },
            {
                "operation": "update",
                "restrictions": {
                    "user": {
                        "results": editor_list,
                        "start": 0,
                        "limit": 100,
                        "size": 1
                    },
                    "group": {
                        "results": [],
                        "start": 0,
                        "limit": 100,
                        "size": 0
                    }
                },
            }
        ],
        "start": 0,
        "limit": 100,
        "size": 2,
    })
}

pub fn sync_restrictions(
    restriction_type: RestrictionType,
    confluence_client: &ConfluenceClient,
    existing_page: &ConfluencePage,
) -> anyhow::Result<()> {
    let body = match restriction_type {
        RestrictionType::SingleEditor(user) => restriction_body(&json!([user])),
        RestrictionType::OpenSpace => restriction_body(&json!([])),
    };
    confluence_client
        .set_restrictions(&existing_page.id, body)?
        .error_for_status()?;
    Ok(())
}
