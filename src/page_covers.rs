use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::str::FromStr;

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::json;
use void::Void;

use crate::local_link::LocalLink;
use crate::{link_generator::LinkGenerator, markdown_page::MarkdownPage};

pub fn string_or_struct<'de, T, D>(deserializer: D) -> std::result::Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = Void>,
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = Void>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<T, E>
        where
            E: serde::de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> std::result::Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cover {
    pub source: Option<String>,
    #[serde(default)]
    pub position: u32,
}

impl Default for Cover {
    fn default() -> Self {
        Self {
            source: None,
            position: 50,
        }
    }
}

impl FromStr for Cover {
    type Err = void::Void;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Cover {
            source: Some(String::from(s)),
            position: 50,
        })
    }
}

pub fn parse_cover(page: &MarkdownPage<'_>, link_generator: &LinkGenerator) -> serde_json::Value {
    if let Some(source) = &page.front_matter.cover.source {
        let position = page.front_matter.cover.position;
        let result = if LocalLink::is_local_link(source) {
            let local_link =
                LocalLink::from_str(source, &PathBuf::from(&page.source)).expect("Should be link");
            json!({"id": link_generator.attachment_id(&local_link.attachment_name(), page).expect("should have attachment id"), "position": position})
        } else {
            json!({"id":source.clone(), "position": position})
        };

        json!(result.to_string()) // wrapped json
    } else {
        serde_json::Value::Null
    }
}

#[cfg(test)]
mod test {
    use assert_fs::fixture::{FileWriteStr, PathChild};
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
        let page = markdown_page_from_str("test.md", PAGE_WITH_COVER_HTTP, &arena)
            .expect("Expected to parse");

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
        let source_defintion = r###"
not_cover: foo
"###;

        let err = saphyr_serde::de::from_str::<Cover>(source_defintion)
            .expect_err("Should not deserialize");

        let err_message = format!("{}", err);
        let expected = "expected `source`";
        assert!(
            err_message.contains(expected),
            "expect '{}' to contain '{}'",
            err_message,
            expected
        );
    }

    #[test]
    fn it_requires_position_to_be_positive() {
        let source_defintion = r###"
position: -50
source: foo.png
     "###;

        let err =
            saphyr_serde::de::from_str::<Cover>(source_defintion).expect_err("Should deserialize");
        let err_message = format!("{}", err);
        let expected = "Unable to parse -50 as a u32";
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
