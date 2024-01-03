use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    html::{format_document_with_plugins, LinkGenerator},
    markdown_space::MarkdownSpace,
    parent::get_parent_title,
};
use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options, Plugins,
};

use crate::{error::ConfluenceError, Result};

pub struct MarkdownPage<'a> {
    pub title: String,
    pub source: String,
    root: &'a AstNode<'a>,
    pub attachments: Vec<PathBuf>,
}

impl<'a> MarkdownPage<'a> {
    pub fn parse(
        markdown_space: &MarkdownSpace,
        markdown_page: &Path,
        arena: &'a Arena<AstNode<'a>>,
    ) -> Result<MarkdownPage<'a>> {
        let content = match fs::read_to_string(markdown_page) {
            Ok(c) => c,
            Err(err) => {
                return Err(ConfluenceError::generic_error(format!(
                    "Failed to read file {}: {}",
                    markdown_page.display(),
                    err
                )))
            }
        };
        Self::parse_content(
            markdown_page,
            &content,
            arena,
            markdown_space
                .relative_page_path(markdown_page)?
                .display()
                .to_string(),
        )
    }

    fn parse_content(
        markdown_page: &Path,
        content: &str,
        arena: &'a Arena<AstNode<'a>>,
        source: String,
    ) -> Result<MarkdownPage<'a>> {
        let mut options = Options::default();
        // options.extension.autolink = true;
        options.extension.table = true;

        let root: &AstNode<'_> = parse_document(arena, content, &options);

        fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
        where
            F: FnMut(&'a AstNode<'a>),
        {
            f(node);
            for c in node.children() {
                iter_nodes(c, f);
            }
        }

        let mut attachments = Vec::<PathBuf>::default();
        let mut first_heading: Option<&AstNode> = None;
        iter_nodes(root, &mut |node| match &mut node.data.borrow_mut().value {
            NodeValue::Heading(_heading) => {
                if first_heading.is_none() {
                    first_heading = Some(node);
                }
            }
            NodeValue::Image(image) => {
                if !image.url.starts_with("http") {
                    let mut attachment_path = PathBuf::from(markdown_page.parent().unwrap());
                    attachment_path.push(image.url.clone());
                    attachments.push(attachment_path);
                }
            }
            _ => (),
        });

        if first_heading.is_none() {
            return Err(ConfluenceError::parsing_error(
                source,
                "missing first heading for title",
            ));
        }

        let heading_node = first_heading.unwrap();
        let mut title = String::default();
        for c in heading_node.children() {
            iter_nodes(c, &mut |child| {
                if let NodeValue::Text(text) = &mut child.data.borrow_mut().value {
                    title += text
                }
            });
        }

        // TODO: it's still allocated tho...
        heading_node.detach();

        Ok(MarkdownPage {
            title,
            source,
            root,
            attachments,
        })
    }

    fn to_html_string(&self, link_generator: &LinkGenerator) -> Result<String> {
        let mut options = Options::default();
        // options.extension.autolink = true;
        options.extension.table = true;
        // options.extension.tasklist = true;
        // options.extension.strikethrough = true;
        let plugins = Plugins::default();

        let mut html = vec![];
        format_document_with_plugins(self.root, &options, &mut html, &plugins, link_generator)
            .unwrap();

        match String::from_utf8(html) {
            Ok(content) => Ok(content),
            Err(_err) => Err(ConfluenceError::generic_error("Failed to convert to utf8")),
        }
    }

    pub fn render(&self, link_generator: &LinkGenerator) -> Result<RenderedPage> {
        let content = self.to_html_string(link_generator)?.clone();
        let title = self.title.clone();
        let page_path = PathBuf::from(self.source.clone());
        let parent = get_parent_title(page_path, link_generator)?;

        Ok(RenderedPage {
            title,
            content,
            source: self.source.clone(),
            parent,
        })
    }
}

#[derive(Debug)]
pub struct RenderedPage {
    pub title: String,
    pub content: String,
    pub source: String,
    pub parent: Option<String>,
}

impl RenderedPage {
    pub fn is_home_page(&self) -> bool {
        self.source == "index.md"
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::error::TestResult;
    use crate::html::LinkGenerator;
    use crate::markdown_page::MarkdownPage;

    #[test]
    fn it_get_first_heading_as_title() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("# My Page Title\n\nMy page content"),
            &arena,
            "page.md".into(),
        )?;

        assert_eq!(page.title, "My Page Title");

        Ok(())
    }

    #[test]
    fn it_removes_title_heading_and_renders_content() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("# My Page Title\n\nMy page content"),
            &arena,
            "page.md".into(),
        )?;

        let content = page.to_html_string(&LinkGenerator::new())?;

        assert!(content.contains("My page content"));
        assert!(!content.contains("<h1>My Page Title</h1>"));

        Ok(())
    }

    #[test]
    fn it_errors_if_no_heading() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &String::from("My page content"),
            &arena,
            "page.md".into(),
        );

        assert!(page.is_err());
        assert_eq!(
            page.err().unwrap().to_string(),
            "Failed to parse page.md: missing first heading for title"
        );

        Ok(())
    }

    fn _it_warns_if_title_and_filename_dont_agree() {}
    fn _it_fails_if_first_non_frontmatter_element_is_not_h1() {}

    #[test]
    fn it_translates_file_links_to_title_links() -> TestResult {
        let link_filename = PathBuf::from("hello-world.md");
        let link_file_title = String::from("This is the title parsed from the linked file");
        let link_text = String::from("Link text");
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            &format!(
                "# My Page Title\n\nMy page content: [{}]({})",
                link_text,
                link_filename.display()
            ),
            &arena,
            "page.md".into(),
        )?;

        let mut link_generator = LinkGenerator::new();

        link_generator.add_file_title(&link_filename, &link_file_title);

        let content = page.to_html_string(&link_generator)?;

        assert!(content.contains(format!("ri:content-title=\"{}\"", link_file_title).as_str()));
        assert!(content.contains(format!("<![CDATA[{}]]>", link_text).as_str()));

        Ok(())
    }

    #[test]
    fn it_skips_unknown_local_links() {}

    #[test]
    fn it_renders_local_file_as_attached_image() -> TestResult {
        // <ac:image>
        //     <ri:attachment ri:filename="atlassian_logo.gif" />
        // </ac:image>
        let arena = Arena::<AstNode>::new();
        let page = MarkdownPage::parse_content(
            PathBuf::from("page.md").as_path(),
            "# My Page Title\n\nMy page content: ![myimage](myimage.png)",
            &arena,
            "page.md".into(),
        )?;

        assert_eq!(page.attachments.len(), 1);

        let content = page.to_html_string(&LinkGenerator::new())?;

        assert!(content.contains(r#"<ri:attachment ri:filename="myimage.png"/>"#));

        Ok(())
    }

    fn _it_raises_an_error_when_image_file_does_not_exist() {}

    #[test]
    fn it_renders_url_as_external_image() {
        // <ac:image>
        //     <ri:url ri:value="http://confluence.atlassian.com/images/logo/confluence_48_trans.png" />
        // </ac:image>
    }
}
