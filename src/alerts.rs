use std::io::{self, Write};

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

#[cfg(test)]
mod test {
    use comrak::{nodes::AstNode, Arena};

    use crate::{error::TestResult, link_generator::LinkGenerator, markdown_page::page_from_str};

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
