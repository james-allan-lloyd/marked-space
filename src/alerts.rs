use std::io::{self, Write};

use comrak::nodes::AlertType;

pub(crate) fn render_expand(
    output: &mut impl Write,
    title: &str,
    entering: bool,
) -> Result<(), io::Error> {
    let actual_title = title.strip_prefix("[expand]").unwrap().trim();
    if entering {
        output.write_all(b"<ac:structured-macro ac:name=\"expand\">")?;
        if !actual_title.is_empty() {
            output.write_all(b"<ac:parameter ac:name=\"title\">")?;
            output.write_all(actual_title.as_bytes())?;
            output.write_all(b"</ac:parameter>")?;
        }
        output.write_all(b"<ac:rich-text-body>")?;
    } else {
        output.write_all(b"</ac:rich-text-body></ac:structured-macro>")?;
    }
    Ok(())
}

fn alert_to_panel_type(alert_type: &AlertType, entering: bool) -> String {
    // > [!NOTE]
    // > Useful information that users should know, even when skimming content.
    //
    // > [!TIP]
    // > Helpful advice for doing things better or more easily.
    //
    // > [!IMPORTANT]
    // > Key information users need to know to achieve their goal.
    //
    // > [!WARNING]
    // > Urgent info that needs immediate user attention to avoid problems.
    //
    // > [!CAUTION]
    // Advises about risks or negative outcomes of certain actions.
    if entering {
        match alert_type {
            AlertType::Note => {
                r#"<ac:structured-macro ac:name="info" ac:schema-version="1" ac:macro-id="eb812e40-8a6b-4e05-a23d-6408d518b775"><ac:rich-text-body>"#.into()
            }
            AlertType::Tip => r#"<ac:structured-macro ac:name="tip" ac:schema-version="1" ac:macro-id="5e263320-f0b8-49c3-ae1b-e058517316d3"><ac:rich-text-body>"#.into(),
            AlertType::Important => {
                r#"<ac:adf-extension><ac:adf-node type="panel"><ac:adf-attribute key="panel-type">note</ac:adf-attribute><ac:adf-content>"#.into()
            }
            AlertType::Warning => r#"<ac:structured-macro ac:name="note" ac:schema-version="1" ac:macro-id="3e4157b1-a25f-4e8f-a9d8-0827b6de0eb2"><ac:rich-text-body>"#.into(),
            AlertType::Caution => r#"<ac:structured-macro ac:name="warning" ac:schema-version="1" ac:macro-id="d7213152-d978-41f1-9963-b9fbc7ed41ad"><ac:rich-text-body>"#.into(),
        }
    } else {
        match alert_type {
            AlertType::Important => "</ac:adf-extension>".into(),
            _ => "</ac:rich-text-body></ac:structured-macro>".into(),
        }
    }
}

pub(crate) fn render_basic_alert(
    output: &mut impl Write,
    node_alert: &comrak::nodes::NodeAlert,
    entering: bool,
) -> Result<(), io::Error> {
    if entering {
        output.write_all(alert_to_panel_type(&node_alert.alert_type, entering).as_bytes())?;
        output.write_all(b"\n<p><strong>")?;
        output.write_all(
            node_alert
                .title
                .as_ref()
                .unwrap_or(&node_alert.alert_type.default_title())
                .as_bytes(),
        )?;
        output.write_all(b"</strong></p>")?;
    } else {
        output.write_all(alert_to_panel_type(&node_alert.alert_type, entering).as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use comrak::{nodes::AstNode, Arena};

    use crate::{error::TestResult, link_generator::LinkGenerator, markdown_page::page_from_str};

    #[test]
    fn it_renders_note() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r###"# compulsory title

> [!note] My Title
>
> Something else after the code

"###;

        let expected_rendered_content = r###"<ac:structured-macro ac:name="info" ac:schema-version="1" ac:macro-id="eb812e40-8a6b-4e05-a23d-6408d518b775"><ac:rich-text-body>
<p><strong>My Title</strong></p>
<p>Something else after the code</p>
</ac:rich-text-body></ac:structured-macro>"###;

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), expected_rendered_content);

        Ok(())
    }

    #[test]
    fn it_renders_default_title() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r###"# compulsory title

> [!note]
>
> Something else after the code

"###;

        let expected_rendered_content = r###"<ac:structured-macro ac:name="info" ac:schema-version="1" ac:macro-id="eb812e40-8a6b-4e05-a23d-6408d518b775"><ac:rich-text-body>
<p><strong>Note</strong></p>
<p>Something else after the code</p>
</ac:rich-text-body></ac:structured-macro>"###;

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), expected_rendered_content);

        Ok(())
    }

    #[test]
    fn it_renders_expand_alert() -> TestResult {
        // note: the [expand] prefix on the title is a bit of a hack; it would be better to maybe
        // fork Comrak and allow Alert parsing to accept non-standard alert types.
        let arena = Arena::<AstNode>::new();
        let markdown_content = r###"# compulsory title

> [!note][expand] My Title
> foo
>
> Something else after the code

"###;

        let expected_rendered_content = r###"<ac:structured-macro ac:name="expand"><ac:parameter ac:name="title">My Title</ac:parameter><ac:rich-text-body>
<p>foo</p>
<p>Something else after the code</p>
</ac:rich-text-body></ac:structured-macro>"###;

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), expected_rendered_content);

        Ok(())
    }

    #[test]
    fn it_renders_expand_alert_with_no_title() -> TestResult {
        // note: the [expand] prefix on the title is a bit of a hack; it would be better to maybe
        // fork Comrak and allow Alert parsing to accept non-standard alert types.
        let arena = Arena::<AstNode>::new();
        let markdown_content = r###"# compulsory title

> [!note][expand]
> foo
>
> Something else after the code

"###;

        let expected_rendered_content = r###"<ac:structured-macro ac:name="expand"><ac:rich-text-body>
<p>foo</p>
<p>Something else after the code</p>
</ac:rich-text-body></ac:structured-macro>"###;

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), expected_rendered_content);

        Ok(())
    }
}
