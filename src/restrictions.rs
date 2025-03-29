use serde_json::json;

use crate::{confluence_client::ConfluenceClient, confluence_page::ConfluencePage};

pub fn sync_restrictions(
    confluence_client: &ConfluenceClient,
    existing_page: &ConfluencePage,
    current_user: &serde_json::Value,
) -> anyhow::Result<()> {
    let body = json!({
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
                        "results": [current_user],
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
    });
    confluence_client
        .set_restrictions(&existing_page.id, body)?
        .error_for_status()?;
    Ok(())
}
