use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::console::{print_status, Status};
use crate::error::Result;
use crate::page_emojis::parse_emoji;
use crate::responses::{self, ContentProperty, MultiEntityResult};
use crate::{
    confluence_client::ConfluenceClient, link_generator::LinkGenerator, markdown_page::MarkdownPage,
};

pub static EMOJI_TITLE_PUBLISHED_PROP: &str = "emoji-title-published";
pub static COVER_PICTURE_ID_PUBLISHED_PROP: &str = "cover-picture-id-published";

fn get_page_property_values(
    page: &MarkdownPage,
    link_generator: &LinkGenerator,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    result.insert(
        String::from(EMOJI_TITLE_PUBLISHED_PROP),
        json!(parse_emoji(page)),
    );

    result.insert(
        String::from(COVER_PICTURE_ID_PUBLISHED_PROP),
        if let Some(cover) = &page.front_matter.cover {
            if MarkdownPage::is_local_link(cover) {
                json!(
                    json!({"id": link_generator.attachment_id(cover, page), "position":50})
                        .to_string()
                )
            } else {
                json!(json!({"id":cover.clone(), "position": 50}).to_string())
            }
        } else {
            serde_json::Value::Null
        },
    );

    result
}

pub fn get_property_updates(
    page: &MarkdownPage<'_>,
    existing_properties: &[ContentProperty],
    link_generator: &LinkGenerator,
) -> Vec<ContentProperty> {
    let mut result = Vec::new();

    let page_properties = get_page_property_values(page, link_generator);
    let mut page_property_keys: HashSet<String> = page_properties.keys().cloned().collect();

    for prop in existing_properties {
        if let Some(new_value) = page_properties.get(&prop.key) {
            if *new_value != prop.value {
                let mut prop_update = prop.clone();
                prop_update.value = new_value.to_owned();
                if !prop_update.value.is_null() {
                    prop_update.version.number += 1;
                }
                result.push(prop_update);
            }
            page_property_keys.remove(&prop.key);
        }
    }

    for prop_to_add in page_property_keys {
        let value_to_add = page_properties.get(&prop_to_add).unwrap();
        if !value_to_add.is_null() {
            result.push(ContentProperty {
                id: String::from(""),
                key: prop_to_add,
                value: value_to_add.clone(),
                version: crate::responses::Version {
                    message: String::from(""),
                    number: 0,
                },
            });
        }
    }

    result
}

pub fn sync_page_properties(
    confluence_client: &ConfluenceClient,
    page: &MarkdownPage,
    page_id: &str,
    link_generator: &LinkGenerator,
) -> Result<()> {
    let prop_json = confluence_client
        .get_properties(page_id)?
        .error_for_status()?
        .json::<MultiEntityResult<responses::ContentProperty>>()?;

    let property_updates = get_property_updates(page, &prop_json.results, link_generator);

    for property_update in property_updates.iter() {
        let update_response = if property_update.value.is_null() {
            print_status(
                Status::Deleted,
                &format!("property {}", &property_update.key),
            );
            confluence_client.delete_property(page_id, &property_update.id)
        } else if property_update.id.is_empty() {
            print_status(
                Status::Created,
                &format!("property {}", &property_update.key),
            );
            confluence_client.create_property(
                page_id,
                json!({"key": property_update.key, "value": property_update.value}),
            )
        } else {
            print_status(
                Status::Updated,
                &format!("property {}", &property_update.key),
            );
            confluence_client.set_property(
                page_id,
                &property_update.id,
                json!({
                    "key": property_update.key,
                    "value": property_update.value,
                    "version": {
                        "message": property_update.version.message,
                        "number": property_update.version.number,
                    }
                }),
            )
        };

        update_response?.error_for_status()?;
    }

    Ok(())
}
