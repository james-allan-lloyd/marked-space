use serde_json::json;

use crate::{markdown_page::MarkdownPage, responses::ContentProperty};

static EMOJI_TITLE_PUBLISHED_PROP: &str = "emoji-title-published";

pub(crate) fn get_property_updates(
    page: &MarkdownPage<'_>,
    existing_properties: &[ContentProperty],
) -> Vec<ContentProperty> {
    let mut result = Vec::new();

    let emoji = parse_emoji(page);

    if let Some(prop) = existing_properties
        .iter()
        .find(|prop| prop.key == EMOJI_TITLE_PUBLISHED_PROP)
    {
        let new_value = json!(emoji);
        if prop.value != new_value {
            let mut prop_update = prop.clone();
            prop_update.value = new_value;
            if emoji.is_some() {
                prop_update.version.number += 1;
            }
            result.push(prop_update);
        }
    } else if emoji.is_some() {
        result.push(ContentProperty {
            id: String::from(""),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!(emoji),
            version: crate::responses::Version {
                message: String::from(""),
                number: 0,
            },
        });
    }

    result
}

pub(crate) fn parse_emoji(page: &MarkdownPage) -> Option<String> {
    let emoji_string = &page.front_matter.emoji;
    if let Some(emoji) = emojis::get_by_shortcode(emoji_string) {
        Some(format!(
            "{:x}",
            emoji.as_str().chars().next().unwrap() as u32
        ))
    } else {
        println!("Unknown short code '{}'", &emoji_string);
        None
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use comrak::{nodes::AstNode, Arena};
    use serde_json::json;

    use crate::{
        markdown_page::page_from_str,
        responses::{ContentProperty, Version},
    };

    use super::*;

    fn without_emoji_md<'a>(
        arena: &'a Arena<comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>>>,
    ) -> MarkdownPage<'a> {
        let markdown_content = r##"---
---
# Test Heading
"##;
        page_from_str("without_emoji.md", markdown_content, arena).unwrap()
    }

    fn heart_eyes_md<'a>(
        arena: &'a Arena<comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>>>,
    ) -> MarkdownPage<'a> {
        let markdown_content = r##"---
emoji:  heart_eyes
---
# Test Heading
"##;
        page_from_str("heart_eyes.md", markdown_content, arena).unwrap()
    }

    #[test]
    fn it_reads_emoji_in_front_matter() {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
emoji:  heart_eyes
---
# Test Heading
"##;
        let page = page_from_str("test.md", markdown_content, &arena).unwrap();
        let emoji = parse_emoji(&page);

        assert_eq!(emoji, Some(String::from_str("1f60d").unwrap()));
    }

    #[test]
    fn it_allows_absent_emojis_in_front_matter() {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
---
# Test Heading
"##;
        let page = page_from_str("test.md", markdown_content, &arena).unwrap();
        let emoji = parse_emoji(&page);

        assert_eq!(emoji, None);
    }

    #[test]
    fn it_fails_if_front_matter_emoji_string_is_not_a_valid_shortcode() {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
emoji:  not_a_short_code
---
# Test Heading
"##;
        let page = page_from_str("test.md", markdown_content, &arena).unwrap();
        let emoji = parse_emoji(&page);

        assert_eq!(emoji, None);
    }

    #[test]
    fn it_adds_emoji_to_pages() {
        // given a confluence page without emojis
        // when I add the emoji field to the front matter
        // then it adds it as a property to the confluence page
        let arena = Arena::<AstNode>::new();
        let page = heart_eyes_md(&arena);

        let existing_properties: Vec<ContentProperty> = Vec::new();

        let property_updates = get_property_updates(&page, &existing_properties);

        let expected_updates = vec![ContentProperty {
            id: String::from(""),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!("1f60d"),
            version: Version {
                number: 0,
                message: String::from(""),
            },
        }];

        assert_eq!(property_updates, expected_updates)
    }

    #[test]
    fn it_removes_emoji_from_pages() {
        // given a confluence page without emojis
        // when I add the emoji field to the front matter
        // then it adds it as a property to the confluence page
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"
# Test Heading
"##;
        let page = page_from_str("test.md", markdown_content, &arena).unwrap();

        let existing_properties: Vec<ContentProperty> = vec![ContentProperty {
            id: String::from("123456"),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!("1f60d"),
            version: Version {
                number: 0,
                message: String::from(""),
            },
        }];

        let property_updates = get_property_updates(&page, &existing_properties);

        let expected_updates = vec![ContentProperty {
            id: String::from("123456"),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!(null),
            version: Version {
                number: 0,
                message: String::from(""),
            },
        }];

        assert_eq!(property_updates, expected_updates)
    }

    #[test]
    fn it_updates_existing_emoji() {
        let arena = Arena::<AstNode>::new();
        let page = heart_eyes_md(&arena);

        let existing_properties: Vec<ContentProperty> = vec![ContentProperty {
            id: String::from("123456"),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!("1f600"), // not heart_eyes
            version: Version {
                number: 0,
                message: String::from(""),
            },
        }];

        let property_updates = get_property_updates(&page, &existing_properties);

        let expected_updates = vec![ContentProperty {
            id: String::from("123456"),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!("1f60d"),
            version: Version {
                number: 1, // version should increment
                message: String::from(""),
            },
        }];

        assert_eq!(property_updates, expected_updates)
    }

    #[test]
    fn it_skips_existing_emoji_updates_when_no_change() {
        let arena = Arena::<AstNode>::new();
        let page = heart_eyes_md(&arena);

        let existing_properties: Vec<ContentProperty> = vec![ContentProperty {
            id: String::from("123456"),
            key: String::from(EMOJI_TITLE_PUBLISHED_PROP),
            value: json!("1f60d"),
            version: Version {
                number: 0,
                message: String::from(""),
            },
        }];

        let property_updates = get_property_updates(&page, &existing_properties);

        let expected_updates: Vec<ContentProperty> = Vec::new();

        assert_eq!(property_updates, expected_updates)
    }

    #[test]
    fn it_skips_absent_emoji_updates_when_no_change() {
        let arena = Arena::<AstNode>::new();
        let page = without_emoji_md(&arena);

        let existing_properties: Vec<ContentProperty> = Vec::new();

        let property_updates = get_property_updates(&page, &existing_properties);

        let expected_updates: Vec<ContentProperty> = Vec::new();

        assert_eq!(property_updates, expected_updates)
    }
}
