// todo: "value": "{\"id\":\"a27ac7fd-5b79-4185-b5a2-c32afe0e84c6\",\"position\":50}",
// "value": "{\"id\":\"https://images.unsplash.com/photo-1541701494587-cb58502866ab?crop=entropy&cs=srgb&fm=jpg&ixid=M3wyMDQ0MDF8MHwxfHNlYXJjaHwzfHxjb2xvcnxlbnwwfDB8fHwxNzQ4Mjk2MDg0fDA&ixlib=rb-4.1.0&q=85\",\"position\":50}",

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use comrak::{nodes::AstNode, Arena};
    use serde_json::json;

    use crate::{
        error::TestResult,
        link_generator::LinkGenerator,
        page_properties::{get_property_updates, COVER_PICTURE_ID_PUBLISHED_PROP},
        responses::ContentProperty,
        test_helpers::markdown_page_from_str,
    };

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
    fn it_errors_if_local_file_is_not_found() {}

    #[test]
    fn it_supports_position() {}

    #[test]
    fn it_supports_fixed_size() {}
}
