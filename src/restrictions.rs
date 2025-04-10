use serde_json::json;

use crate::{
    confluence_client::ConfluenceClient, confluence_page::ConfluencePage, console::print_status,
};

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
    let existing_restrictions = confluence_client
        .get_restrictions_by_operation(&existing_page.id)?
        .error_for_status()?
        .json::<serde_json::Value>()?;

    let updated = match restriction_type {
        RestrictionType::SingleEditor(user) => {
            let update = should_update_restrictions(user, &existing_restrictions)?;
            if update {
                let users = json!([user]);
                let body = restriction_body(&users);
                print_status(crate::console::Status::Updated, "permissions");
                Some(confluence_client.set_restrictions(&existing_page.id, body)?)
            } else {
                None
            }
        }

        RestrictionType::OpenSpace => None,
    };
    if let Some(response) = updated {
        if !response.status().is_success() {
            println!("{}", &response.text()?);
            return Err(anyhow::anyhow!("Not able to update restrictions"));
        }
    }
    Ok(())
}

fn should_update_restrictions(
    user: &serde_json::Value,
    existing_restrictions: &serde_json::Value,
) -> Result<bool, anyhow::Error> {
    let existing_users_json = existing_restrictions.pointer("/update/restrictions/user/results");
    let mut update = false;
    if let Some(existing_users) = existing_users_json {
        let a = existing_users
            .as_array()
            .ok_or(anyhow::anyhow!("Missing users array"))?;
        if a.len() != 1 || a[0]["accountId"].as_str() != user["accountId"].as_str() {
            update = true;
        }
        Ok(update)
    } else {
        Err(anyhow::anyhow!("Missing results"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{error::TestResult, restrictions::should_update_restrictions};

    fn by_operation_body() -> serde_json::Value {
        json!({
          "read": {
            "operation": "read",
            "restrictions": {
              "user": {
                "results": [],
                "start": 0,
                "limit": 200,
                "size": 0
              },
              "group": {
                "results": [],
                "start": 0,
                "limit": 200,
                "size": 0
              }
            },
          },
          "update": {
            "operation": "update",
            "restrictions": {
              "user": {
                "results": [
                ],
                "start": 0,
                "limit": 200,
                "size": 1
              },
              "group": {
                "results": [],
                "start": 0,
                "limit": 200,
                "size": 0
              }
            },
          },
        })
    }

    #[test]
    fn it_errors_if_data_not_present() {}

    #[test]
    fn it_updates_if_has_user_but_not_the_current() -> TestResult {
        let user = json!({
            "accountId": "foobarbaz",
        });
        let other_user = json!({
            "accountId": "barry",
        });
        let mut current_restrictions = by_operation_body();
        current_restrictions["update"]["restrictions"]["user"]["results"] = json!([other_user]);
        assert!(should_update_restrictions(&user, &current_restrictions)?);
        Ok(())
    }

    #[test]
    fn it_does_not_update_if_user_is_already_current() -> TestResult {
        let user = json!({
            "accountId": "foobarbaz",
        });
        let mut current_restrictions = by_operation_body();
        current_restrictions["update"]["restrictions"]["user"]["results"] = json!([user]);
        assert!(!should_update_restrictions(&user, &current_restrictions)?);
        Ok(())
    }

    #[test]
    fn it_updates_not_the_sole_user() -> TestResult {
        let user = json!({
            "accountId": "foobarbaz",
        });
        let other_user = json!({
            "accountId": "barry",
        });
        let mut current_restrictions = by_operation_body();
        current_restrictions["update"]["restrictions"]["user"]["results"] =
            json!([user, other_user]);
        assert!(should_update_restrictions(&user, &current_restrictions)?);
        Ok(())
    }

    #[test]
    fn it_does_nothing_in_openspace_mode() {
        // assume that permissions are managed by the user in openspace mode
    }
}
