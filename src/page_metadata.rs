#[cfg(test)]
mod test {
    use comrak::{nodes::AstNode, Arena};

    use crate::{error::TestResult, link_generator::LinkGenerator, markdown_page::page_from_str};

    #[test]
    fn it_allows_metadata() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
metadata:
    some:
        arbitrary: "value"
---
# compulsory title
"##;

        let page = page_from_str("page.md", markdown_content, &arena)?;

        assert_eq!(
            page.front_matter.metadata["some"]["arbitrary"].as_str(),
            Some("value")
        );

        Ok(())
    }

    #[test]
    fn it_has_metadata_as_variables() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
metadata:
    some:
        arbitrary: "value"
---
# compulsory title
{{ metadata(path="some.arbitrary") }}
"##;
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let rendered_page = page.render(&LinkGenerator::default_test())?;

        assert_eq!(rendered_page.content.trim(), "<p>value</p>");

        Ok(())
    }
}
