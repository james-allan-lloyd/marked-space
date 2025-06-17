use std::{
    fs::File,
    io::{self},
    path::{Path, PathBuf},
};

use crate::{
    attachments::Attachment, checksum::sha256_digest, confluence_page::ConfluencePageData,
    confluence_storage_renderer::render_confluence_storage, frontmatter::FrontMatter,
    helpers::collect_text, link_generator::LinkGenerator, local_link::LocalLink,
    parent::get_parent_file, template_renderer::TemplateRenderer,
};
use anyhow::Context;
use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};

use crate::{error::ConfluenceError, Result};

pub struct MarkdownPage<'a> {
    pub title: String,
    pub source: String,
    root: &'a AstNode<'a>,
    pub attachments: Vec<Attachment>,
    pub local_links: Vec<LocalLink>,
    pub front_matter: FrontMatter,
    pub warnings: Vec<String>,
}

pub fn remove_prefix(prefix: &Path, page_path: &Path) -> Result<String> {
    let space_relative_path = page_path.strip_prefix(prefix).map_err(|_e| {
        ConfluenceError::generic_error(format!(
            "Page is not in space directory {}: {}",
            prefix.display(),
            page_path.display()
        ))
    })?;

    Ok(PathBuf::from(space_relative_path)
        .to_str()
        .ok_or(ConfluenceError::generic_error(
            "Failed to convert path to str",
        ))?
        .replace('\\', "/"))
}

impl<'a> MarkdownPage<'a> {
    pub fn from_file(
        // markdown_space: &MarkdownSpace,
        space_dir: &Path,
        markdown_page: &Path,
        arena: &'a Arena<AstNode<'a>>,
        template_renderer: &mut TemplateRenderer,
    ) -> Result<MarkdownPage<'a>> {
        let source_string = remove_prefix(space_dir, markdown_page)?;
        // let markdown_page = space_dir.join(source);
        let file = File::open(markdown_page)?;
        let mut reader = io::BufReader::new(file);
        let (fm, original_content) =
            FrontMatter::from_reader(&mut reader).with_context(|| source_string.clone())?;

        let content = template_renderer
            .render_template_str(&source_string, &original_content, &fm)
            .context(format!("Loading markdown from file {}", source_string))?;
        Self::parse_markdown(arena, source_string, markdown_page, &content, fm)
    }

    #[cfg(test)]
    pub fn from_str(
        markdown_page: &Path,
        content: &str,
        arena: &'a Arena<AstNode<'a>>,
        source: String,
        template_renderer: &mut TemplateRenderer,
    ) -> Result<MarkdownPage<'a>> {
        let (fm, original_content) = FrontMatter::from_str(content)?;
        let content = template_renderer
            .render_template_str(source.as_str(), &original_content, &fm)
            .context(format!("Failed to render markdown from file {}", source))?;
        Self::parse_markdown(arena, source, markdown_page, &content, fm)
    }

    fn options() -> Options<'a> {
        let mut options = Options::default();
        options.render.unsafe_ = true;
        // options.extension.autolink = true;
        options.extension.table = true;
        options.extension.tasklist = true;
        options.extension.strikethrough = true;
        options.extension.front_matter_delimiter = Some("---".to_string());
        options.extension.shortcodes = true;
        options.extension.tagfilter = true;
        options.extension.alerts = true;
        options
    }

    fn parse_markdown(
        arena: &'a Arena<AstNode<'a>>,
        source: String,
        markdown_page: &Path,
        content: &str,
        fm: FrontMatter,
    ) -> Result<MarkdownPage<'a>> {
        let parent = markdown_page.parent().unwrap();
        let root: &AstNode<'_> = parse_document(arena, content, &Self::options());

        fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
        where
            F: FnMut(&'a AstNode<'a>),
        {
            f(node);
            for c in node.children() {
                iter_nodes(c, f);
            }
        }

        let mut errors = Vec::<String>::default();
        let mut warnings = Vec::<String>::default();
        if !fm.unknown_keys.is_empty() {
            warnings.push(format!(
                "Unknown top level front matter keys: {}",
                fm.unknown_keys.join(", "),
            ));
        }

        let mut attachments = Vec::<Attachment>::default();
        if let Some(source) = &fm.cover.source {
            if LocalLink::is_local_link(source) {
                attachments.push(Attachment::image(source, parent));
            }
        }
        let mut local_links = Vec::<LocalLink>::default();
        let mut first_heading: Option<&AstNode> = None;
        iter_nodes(root, &mut |node| match &mut node.data.borrow_mut().value {
            NodeValue::Heading(_heading) => {
                if first_heading.is_none() {
                    first_heading = Some(node);
                } else {
                    let mut text_content = Vec::with_capacity(20);
                    for n in node.children() {
                        collect_text(n, &mut text_content);
                    }
                }
            }
            NodeValue::Image(image) => {
                if LocalLink::is_local_link(&image.url) {
                    attachments.push(Attachment::image(&image.url, parent));
                }
            }
            NodeValue::Link(node_link) => {
                if LocalLink::is_local_link(&node_link.url) {
                    if let Ok(local_link) = LocalLink::from_str(&node_link.url, parent) {
                        if local_link.is_page() {
                            local_links.push(local_link);
                        } else {
                            attachments.push(Attachment::file(&local_link));
                        }
                    } else {
                        errors.push(format!("Failed to parse local link: {}", node_link.url));
                    }
                } else {
                    // remote link
                }
            }
            _ => (),
        });

        let mut title = String::default();

        if let Some(heading_node) = first_heading {
            if let NodeValue::Heading(heading) = heading_node.data.borrow().value {
                if heading.level != 1 {
                    errors.push(format!(
                        "first heading in file should be level 1, instead was level {}",
                        heading.level
                    ));
                }
            }
            let mut output = Vec::default();
            collect_text(heading_node, &mut output);
            title = String::from_utf8(output)?;

            // TODO: it's still allocated tho...
            heading_node.detach();
        } else {
            errors.push(String::from("missing first heading for title"));
        }

        if errors.is_empty() {
            Ok(MarkdownPage {
                title,
                source,
                root,
                attachments,
                local_links,
                warnings,
                front_matter: fm,
            })
        } else {
            Err(ConfluenceError::parsing_errors(source, errors))
        }
    }

    fn to_html_string(&self, link_generator: &LinkGenerator) -> Result<String> {
        let mut html = vec![];
        render_confluence_storage(
            self.root,
            &Self::options(),
            &mut html,
            link_generator,
            &PathBuf::from(self.source.clone()),
        )
        .unwrap();

        match String::from_utf8(html) {
            Ok(content) => Ok(content),
            Err(_err) => Err(ConfluenceError::generic_error("Failed to convert to utf8")),
        }
    }

    pub fn render(&self, link_generator: &LinkGenerator) -> Result<RenderedPage> {
        let rendered_html = self.to_html_string(link_generator)?.clone();
        let content = rendered_html;
        let title = self.title.clone();
        let page_path = PathBuf::from(self.source.clone());
        let parent = get_parent_file(&page_path).and_then(|f| link_generator.get_file_id(&f));
        let checksum = sha256_digest(content.as_bytes())?;

        Ok(RenderedPage {
            title,
            content,
            source: self.source.clone(),
            parent,
            checksum,
        })
    }

    pub(crate) fn is_folder(&self) -> bool {
        self.front_matter.folder
    }
}

#[derive(Debug)]
pub struct RenderedPage {
    pub title: String,
    pub content: String,
    pub source: String,
    pub parent: Option<String>,
    pub checksum: String,
}

impl RenderedPage {
    pub fn is_home_page(&self) -> bool {
        self.source == "index.md"
    }

    pub fn version_message(&self) -> String {
        format!(
            "{} source={}; checksum={}",
            ConfluencePageData::version_message_prefix(),
            self.source.replace('\\', "/"), // needs to be platform independent
            self.checksum
        )
    }
}

#[cfg(test)]
pub fn page_from_str<'a>(
    filename: &str,
    content: &str,
    arena: &'a Arena<AstNode<'a>>,
) -> crate::error::Result<MarkdownPage<'a>> {
    MarkdownPage::from_str(
        &PathBuf::from(filename),
        content,
        arena,
        filename.to_string(),
        &mut TemplateRenderer::default()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData};
    use crate::error::TestResult;
    use crate::link_generator::LinkGenerator;
    use crate::markdown_page::LocalLink;
    use crate::responses::{ContentStatus, Version};

    #[test]
    fn it_get_first_heading_as_title() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = &String::from("# My Page Title\n\nMy page content");
        let page = page_from_str("page.md", markdown_content, &arena)?;

        assert_eq!(page.title, "My Page Title");

        Ok(())
    }

    #[test]
    fn it_removes_title_heading_and_renders_content() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = &String::from("# My Page Title\n\nMy page content");
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let content = page.to_html_string(&LinkGenerator::default_test())?;

        assert!(content.contains("My page content"));
        assert!(!content.contains("<h1>My Page Title</h1>"));

        Ok(())
    }

    #[test]
    fn it_errors_if_no_heading() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = &String::from("My page content");
        let page = page_from_str("page.md", markdown_content, &arena);

        assert!(page.is_err());
        assert_eq!(
            page.err().unwrap().to_string(),
            "Failed to parse page.md: missing first heading for title"
        );

        Ok(())
    }

    #[test]
    fn it_fails_if_first_non_frontmatter_element_is_not_h1() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = &String::from("## First Heading Needs to be H1");
        let page = page_from_str("page.md", markdown_content, &arena);

        assert!(page.is_err());
        assert_eq!(
            page.err().unwrap().to_string(),
            "Failed to parse page.md: first heading in file should be level 1, instead was level 2"
        );

        Ok(())
    }

    #[test]
    fn it_parses_file_links_with_anchors() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let link_filename = PathBuf::from("some-page.md");
        let markdown_content = &format!(
            "# My Page Title\n\nMy page content: [link text]({}#some-anchor)",
            link_filename.display()
        );
        let page = page_from_str("page.md", markdown_content, &arena)?;
        assert_eq!(
            page.local_links,
            vec![LocalLink {
                text: format!("{:#}#some-anchor", link_filename.display().to_string()),
                target: link_filename,
                page_path: PathBuf::default(), // from("some-page.md"),
                anchor: Some(String::from("some-anchor"))
            }]
        );

        Ok(())
    }

    fn dummy_confluence_page(title: &str, id: &str) -> ConfluenceNode {
        ConfluenceNode {
            id: id.to_string(),
            title: title.to_string(),
            parent_id: None,
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::default(),
                    number: 1,
                },
                path: None, // "foo.md".to_string(),
                status: ContentStatus::Current,
            }),
        }
    }

    #[test]
    fn it_uses_the_title_when_linktext_is_empty() -> TestResult {
        let link_filename = PathBuf::from("hello-world.md");
        let link_text = String::from("");
        let link_url = String::from("https://example.atlassian.net/wiki/spaces/TEST/pages/47");
        let arena = Arena::<AstNode>::new();
        let markdown_content = &format!(
            "# My Page Title\n\nMy page content: [{}]({})",
            link_text,
            link_filename.display()
        );
        let page = page_from_str("page.md", markdown_content, &arena)?;
        let linked_page = page_from_str(
            link_filename.as_os_str().to_str().unwrap(),
            "# A Linked Page\n",
            &arena,
        )?;

        let mut link_generator = LinkGenerator::default_test();

        link_generator.register_markdown_page(&page)?;
        link_generator.register_markdown_page(&linked_page)?;
        link_generator.register_confluence_node(&dummy_confluence_page("A Linked Page", "47"));

        let content = page.to_html_string(&link_generator)?;
        let expected = format!("<a href=\"{}\">{}</a>", link_url, "A Linked Page");
        assert!(content.contains(expected.as_str()));

        Ok(())
    }

    #[test]
    fn it_translates_file_links() -> TestResult {
        let link_filename = PathBuf::from("hello-world.md");
        let link_text = String::from("Link text");
        let link_url = String::from("https://example.atlassian.net/wiki/spaces/TEST/pages/47");
        let arena = Arena::<AstNode>::new();
        let markdown_content = &format!(
            "# My Page Title\n\nMy page content: [{}](./hello-world.md)",
            link_text,
            // link_filename.display()
        );
        let page = page_from_str("page.md", markdown_content, &arena)?;
        let linked_page = page_from_str(
            link_filename.as_os_str().to_str().unwrap(),
            "# A Linked Page\n",
            &arena,
        )?;

        let mut link_generator = LinkGenerator::default_test();

        link_generator.register_markdown_page(&page)?;
        link_generator.register_markdown_page(&linked_page)?;
        link_generator.register_confluence_node(&dummy_confluence_page("A Linked Page", "47"));

        let content = page.to_html_string(&link_generator)?;
        let expected = format!("<a href=\"{}\">{}</a>", link_url, link_text);
        assert!(content.contains(expected.as_str()));

        Ok(())
    }

    #[test]
    fn it_translates_relative_file_links() -> TestResult {
        let arena = Arena::<AstNode>::new();

        let link_filename = PathBuf::from("hello-world.md");
        let link_text = String::from("Link text");
        let link_url = String::from("https://example.atlassian.net/wiki/spaces/TEST/pages/47");
        let markdown_content = &format!(
            "# My Page Title\n\nMy page content: [{}](./hello-world.md)", // should handle
            // relative paths
            link_text,
        );
        let page = page_from_str("page.md", markdown_content, &arena)?;
        let linked_page = page_from_str(
            link_filename.as_os_str().to_str().unwrap(),
            "# A Linked Page\n",
            &arena,
        )?;

        let mut link_generator = LinkGenerator::default_test();

        link_generator.register_markdown_page(&page)?;
        link_generator.register_markdown_page(&linked_page)?;
        link_generator.register_confluence_node(&dummy_confluence_page("A Linked Page", "47"));

        let content = page.to_html_string(&link_generator)?;
        let expected = format!("<a href=\"{}\">{}</a>", link_url, link_text);
        assert!(content.contains(expected.as_str()));

        Ok(())
    }

    #[test]
    fn it_renders_local_file_as_attached_image() -> TestResult {
        let content = "# My Page Title\n\nMy page content: ![myimage](myimage.png)";
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", content, &arena)?;

        assert_eq!(page.attachments.len(), 1);

        let content = page.to_html_string(&LinkGenerator::default_test())?;

        assert!(content.contains(r#"<ri:attachment ri:filename="myimage.png"/>"#));

        Ok(())
    }

    #[test]
    fn it_renders_url_as_external_image() -> TestResult {
        // <ac:image>
        //     <ri:url ri:value="http://confluence.atlassian.com/images/logo/confluence_48_trans.png" />
        // </ac:image>
        let image_url = "http://www.example.com/image.png";
        let markdown_content = format!(
            "# My Page Title\n\nMy page content: ![myimage]({})",
            image_url
        );
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", markdown_content.as_str(), &arena)?;

        assert_eq!(page.attachments.len(), 0); // should not view the external link as an attachment
        let html_content = page.to_html_string(&LinkGenerator::default_test())?;

        assert!(html_content.contains(
            format!(
                r#"<ac:image ac:align="center"><ri:url ri:value="{}"/>myimage</ac:image>"#,
                image_url
            )
            .as_str()
        ));

        Ok(())
    }

    #[test]
    fn it_renders_links_to_external_pages() -> TestResult {
        let external_url = "https://example.com";
        let markdown_content = format!(
            "# My Page Title\n\nExternal Link: [example]({})",
            external_url
        );
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", markdown_content.as_str(), &arena)?;
        let html_content = page.to_html_string(&LinkGenerator::default_test())?;

        let expected = format!(r#"<a href="{}">example</a>"#, external_url);

        assert!(
            html_content.contains(&expected),
            "expected {} to contain {}",
            html_content,
            expected
        );

        Ok(())
    }

    #[test]
    fn it_renders_templates() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{filename}}";
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let rendered_page = page.render(&LinkGenerator::default_test())?;

        assert_eq!(rendered_page.content.trim(), "<p>page.md</p>");

        Ok(())
    }

    #[test]
    fn it_parses_yaml_frontmatter() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = r##"---
labels: 
- foo
- bar
---
# compulsory title
"##;

        let page = page_from_str("page.md", markdown_content, &arena)?;

        let expected_labels = vec!["foo", "bar"];

        assert_eq!(page.front_matter.labels, expected_labels);

        Ok(())
    }
}
