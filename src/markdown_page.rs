use std::{fs, path::Path};

use comrak::{
    format_html,
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};

use crate::{error::ConfluenceError, Result};

pub struct MarkdownPage<'a> {
    pub title: String,
    pub source: String,
    root: &'a AstNode<'a>,
}

impl<'a> MarkdownPage<'a> {
    pub fn parse(markdown_page: &Path, arena: &'a Arena<AstNode<'a>>) -> Result<MarkdownPage<'a>> {
        let content = match fs::read_to_string(markdown_page) {
            Ok(c) => c,
            Err(err) => {
                return Err(ConfluenceError::generic_error(format!(
                    "Failed to read file {}: {}",
                    markdown_page.display(),
                    err.to_string()
                )))
            }
        };
        Self::parse_content(markdown_page, &content, arena)
    }

    fn parse_content(
        markdown_page: &Path,
        content: &String,
        arena: &'a Arena<AstNode<'a>>,
    ) -> Result<MarkdownPage<'a>> {
        let source = markdown_page.display().to_string();

        let root: &AstNode<'_> = parse_document(arena, content.as_str(), &Options::default());

        fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
        where
            F: FnMut(&'a AstNode<'a>),
        {
            f(node);
            for c in node.children() {
                iter_nodes(c, f);
            }
        }

        let mut first_heading: Option<String> = None;
        // TODO: get reference to node during iteration, then remove it

        iter_nodes(root, &mut |node| match &mut node.data.borrow_mut().value {
            NodeValue::Heading(_heading) => {
                if first_heading.is_none() {
                    let mut heading_text = String::default();
                    // TODO: this is a double iteration of children
                    for c in node.children() {
                        iter_nodes(c, &mut |child| match &mut child.data.borrow_mut().value {
                            NodeValue::Text(text) => {
                                println!("heading text {}", text);
                                heading_text += text
                            }
                            _ => (),
                        });
                    }
                    first_heading = Some(heading_text);
                }
            }
            _ => (),
        });

        if first_heading.is_none() {
            return Err(ConfluenceError::generic_error(format!(
                "Couldn't find heading in [{}]",
                source
            )));
        }

        let title = first_heading.unwrap();

        Ok(MarkdownPage {
            title,
            source,
            root,
        })
    }

    pub fn to_html_string(&self) -> Result<String> {
        let mut html = vec![];
        format_html(&self.root, &Options::default(), &mut html).unwrap();

        match String::from_utf8(html) {
            Ok(content) => Ok(content),
            Err(_err) => Err(ConfluenceError::generic_error("Failed to convert to utf8")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::markdown_page::MarkdownPage;
    use crate::Result;

    #[test]
    fn it_get_first_heading_as_title() -> Result<()> {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("# My Page Title\n\nMy page content"),
            &arena,
        )?;

        assert_eq!(page.title, "My Page Title");

        Ok(())
    }

    #[test]
    fn it_renders_content() -> Result<()> {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("# My Page Title\n\nMy page content"),
            &arena,
        )?;

        let content = page.to_html_string()?;

        assert!(content.contains("<h1>My Page Title</h1>"));

        Ok(())
    }

    #[test]
    fn it_errors_if_no_heading() -> Result<()> {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("My page content"),
            &arena,
        );

        assert!(page.is_err());
        assert_eq!(
            page.err().unwrap().to_string(),
            "error: Couldn't find heading in [page.md]"
        );

        Ok(())
    }

    fn _it_warns_if_title_and_filename_dont_agree() {}
    fn _it_fails_if_first_non_frontmatter_element_is_not_h1() {}
}
