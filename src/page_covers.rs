use saphyr::Yaml;
use serde_json::json;

use crate::error::Result;
use crate::{link_generator::LinkGenerator, markdown_page::MarkdownPage};
use anyhow::anyhow;

#[derive(Debug, PartialEq, Eq)]
pub struct Cover {
    pub source: String,
    pub position: u32,
}

fn position_checked(value: i64) -> Result<u32> {
    if value < 0 {
        Err(anyhow!("cover.position must not be negative"))
    } else if value > u32::MAX as i64 {
        Err(anyhow!("cover.position is too large"))
    } else {
        Ok(value as u32)
    }
}

impl Cover {
    pub fn from_yaml(yaml: &Yaml) -> Result<Option<Self>> {
        match yaml {
            Yaml::String(source) => Ok(Some(Cover {
                source: source.clone(),
                position: 50,
            })),
            Yaml::Hash(_hash) => {
                let source = yaml["source"]
                    .as_str()
                    .ok_or(anyhow!("cover.source is missing or not a string"))?;
                let position = position_checked(yaml["position"].as_i64().unwrap_or(50))?;

                Ok(Some(Cover {
                    source: String::from(source),
                    position,
                }))
            }
            Yaml::BadValue => Ok(None),
            _ => Err(anyhow!("Invalid type for cover: {:?}", yaml)),
        }
    }
}

// todo: "value": "{\"id\":\"a27ac7fd-5b79-4185-b5a2-c32afe0e84c6\",\"position\":50}",
// "value": "{\"id\":\"https://images.unsplash.com/photo-1541701494587-cb58502866ab?crop=entropy&cs=srgb&fm=jpg&ixid=M3wyMDQ0MDF8MHwxfHNlYXJjaHwzfHxjb2xvcnxlbnwwfDB8fHwxNzQ4Mjk2MDg0fDA&ixlib=rb-4.1.0&q=85\",\"position\":50}",
pub fn parse_cover(page: &MarkdownPage<'_>, link_generator: &LinkGenerator) -> serde_json::Value {
    if let Some(cover) = &page.front_matter.cover {
        let result = if MarkdownPage::is_local_link(&cover.source) {
            json!({"id": link_generator.attachment_id(&cover.source, page), "position":cover.position})
        } else {
            json!({"id":cover.source.clone(), "position": 50})
        };

        json!(result.to_string()) // wrapped json
    } else {
        serde_json::Value::Null
    }
}

#[cfg(test)]
mod test {
    use assert_fs::fixture::{FileWriteStr, PathChild};
    use saphyr::Yaml;
    use std::str::FromStr;

    use comrak::{nodes::AstNode, Arena};
    use serde_json::json;

    use crate::{
        confluence_client::ConfluenceClient,
        error::TestResult,
        link_generator::LinkGenerator,
        markdown_space::MarkdownSpace,
        page_properties::{get_property_updates, COVER_PICTURE_ID_PUBLISHED_PROP},
        responses::ContentProperty,
        sync::sync_space,
        test_helpers::markdown_page_from_str,
        Args,
    };

    use super::{parse_cover, Cover};

    fn unwrap_value(value: &serde_json::Value) -> Result<tera::Value, serde_json::Error> {
        serde_json::Value::from_str(value.as_str().unwrap())
    }

    const PAGE_WITH_COVER_HTTP: &str = r###"---
cover: https://example.com/image.png
---
# A Page with a Cover (HTTP)
"###;

    const PAGE_WITH_COVER_LOCAL_FILE: &str = r###"---
cover: image.png
---
# A Page with a Cover (LOCAL)
"###;

    const PAGE_WITH_POSITION: &str = r###"---
cover:
    source: image.png
    position: 0
---
# A Page with a Cover (LOCAL)
"###;

    #[test]
    fn it_adds_cover_from_http() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = markdown_page_from_str("test.md", PAGE_WITH_COVER_HTTP, &arena)?;

        assert!(page.attachments.is_empty());

        let existing_properties: Vec<ContentProperty> = Vec::new();
        let property_updates =
            get_property_updates(&page, &existing_properties, &LinkGenerator::default_test());

        let expected_value = json!({"id": "https://example.com/image.png", "position": 50});

        assert_eq!(property_updates.len(), 1);
        assert_eq!(property_updates[0].key, COVER_PICTURE_ID_PUBLISHED_PROP);
        assert_eq!(unwrap_value(&property_updates[0].value)?, expected_value);

        Ok(())
    }

    #[test]
    fn it_adds_cover_from_local_file() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = markdown_page_from_str("test.md", PAGE_WITH_COVER_LOCAL_FILE, &arena)?;

        assert_eq!(page.attachments.len(), 1, "Should add cover as attachment");

        let mut link_generator = LinkGenerator::default_test();
        link_generator.register_attachment_id(&page.source, "image.png", "some_other_id");

        let existing_properties: Vec<ContentProperty> = Vec::new();
        let property_updates = get_property_updates(&page, &existing_properties, &link_generator);

        let expected_value = json!({"id": "some_other_id", "position": 50});

        assert_eq!(property_updates.len(), 1);
        assert_eq!(property_updates[0].key, COVER_PICTURE_ID_PUBLISHED_PROP);
        assert_eq!(unwrap_value(&property_updates[0].value)?, expected_value);

        Ok(())
    }

    #[test]
    fn it_errors_if_local_file_is_not_found() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str(PAGE_WITH_COVER_LOCAL_FILE)
            .unwrap();

        let confluence_client = ConfluenceClient::new("host.example.com");
        let mut space = MarkdownSpace::from_directory(temp.child("test").path())?;
        let sync_result = sync_space(confluence_client, &mut space, Args::default());

        assert!(sync_result.is_err());

        let expected_string =
            String::from("Missing file for attachment link in [index.md] to [image.png]");
        let error_string = sync_result.unwrap_err().to_string();

        assert!(
            error_string.contains(&expected_string),
            "Unexpected error: [{}], should contain [{}]",
            error_string,
            expected_string
        );

        Ok(())
    }

    #[test]
    fn it_supports_position() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = markdown_page_from_str("test.md", PAGE_WITH_POSITION, &arena)?;

        let mut link_generator = LinkGenerator::default_test();
        link_generator.register_attachment_id(&page.source, "image.png", "some_other_id");
        let cover_value = parse_cover(&page, &link_generator);

        assert_eq!(
            unwrap_value(&cover_value)?,
            json!({"id": "some_other_id", "position": 0})
        );

        Ok(())
    }

    #[test]
    fn it_requires_source_key() {
        let source_defintion = Yaml::load_from_str(
            r###"
cover:
    position: 50
    not_cover: foo
"###,
        )
        .unwrap()
        .clone();

        let result = Cover::from_yaml(&source_defintion[0]["cover"]);
        assert!(result.is_err());
        let err_message = format!("{}", result.err().unwrap());
        let expected = "cover.source is missing or not a string";
        assert!(
            err_message.contains(expected),
            "expect '{}' to contain '{}'",
            err_message,
            expected
        );
    }

    #[test]
    fn it_requires_position_to_be_positive() {
        let source_defintion = Yaml::load_from_str(
            r###"
cover:
    position: -50
    source: foo.png
"###,
        )
        .unwrap()
        .clone();
        let result = Cover::from_yaml(&source_defintion[0]["cover"]);

        assert!(result.is_err());
        let err_message = format!("{}", result.err().unwrap());
        let expected = "position must not be negative";
        assert!(
            err_message.contains(expected),
            "expect '{}' to contain '{}'",
            err_message,
            expected
        );
    }

    #[test]
    fn it_supports_fixed_size() {
        // this is a graphql call :(
    }
}
