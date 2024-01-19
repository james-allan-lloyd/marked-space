use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    checksum::sha256_digest, confluence_page::ConfluencePage,
    confluence_storage_renderer::render_confluence_storage, helpers::collect_text,
    link_generator::LinkGenerator, local_link::LocalLink, markdown_space::MarkdownSpace,
    parent::get_parent_file, template_renderer::TemplateRenderer,
};
use anyhow::Context;
use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};
use serde::Deserialize;

use crate::{error::ConfluenceError, Result};

#[derive(Deserialize, Debug, PartialEq)]
pub struct FrontMatter {
    pub labels: Vec<String>,
}

pub struct MarkdownPage<'a> {
    pub title: String,
    pub source: String,
    root: &'a AstNode<'a>,
    pub attachments: Vec<PathBuf>,
    pub local_links: Vec<LocalLink>,
    pub front_matter: Option<FrontMatter>,
}

impl<'a> MarkdownPage<'a> {
    pub fn from_file(
        markdown_space: &MarkdownSpace,
        markdown_page: &Path,
        arena: &'a Arena<AstNode<'a>>,
        template_renderer: &mut TemplateRenderer,
    ) -> Result<MarkdownPage<'a>> {
        let source = markdown_space
            .relative_page_path(markdown_page)?
            .display()
            .to_string();
        let content = template_renderer
            .render_template(&source)
            .context(format!("Loading markdown from file {}", source))?;
        Self::parse_markdown(arena, source, markdown_page, &content)
    }

    pub fn from_str(
        markdown_page: &Path,
        content: &str,
        arena: &'a Arena<AstNode<'a>>,
        source: String,
        template_renderer: &mut TemplateRenderer,
    ) -> Result<MarkdownPage<'a>> {
        let content = template_renderer.expand_html_str(source.as_str(), content)?;
        Self::parse_markdown(arena, source, markdown_page, &content)
    }

    fn options() -> Options {
        let mut options = Options::default();
        options.render.unsafe_ = true;
        // options.extension.autolink = true;
        options.extension.table = true;
        options.extension.tasklist = true;
        options.extension.strikethrough = true;
        options.extension.front_matter_delimiter = Some("---".to_string());
        options.extension.shortcodes = true;
        options.extension.tagfilter = true;
        options
    }

    fn parse_markdown(
        arena: &'a Arena<AstNode<'a>>,
        source: String,
        markdown_page: &Path,
        content: &str,
    ) -> Result<MarkdownPage<'a>> {
        let parent = markdown_page.parent().unwrap();
        let root: &AstNode<'_> = parse_document(arena, &content, &Self::options());

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
        let mut attachments = Vec::<PathBuf>::default();
        let mut local_links = Vec::<LocalLink>::default();
        let mut first_heading: Option<&AstNode> = None;
        let mut front_matter: Option<FrontMatter> = None;
        iter_nodes(root, &mut |node| match &mut node.data.borrow_mut().value {
            NodeValue::FrontMatter(front_matter_str) => {
                let front_matter_str = front_matter_str
                    .trim()
                    .strip_prefix("---")
                    .unwrap()
                    .strip_suffix("---")
                    .unwrap();
                match serde_yaml::from_str(front_matter_str) {
                    Ok(front_matter_yaml) => {
                        front_matter = Some(front_matter_yaml);
                    }
                    Err(err) => {
                        errors.push(format!("Couldn't parse front matter: {}", err));
                    }
                }
            }
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
                if !image.url.starts_with("http") {
                    let mut attachment_path = PathBuf::from(parent);
                    attachment_path.push(image.url.clone());
                    attachments.push(attachment_path);
                }
            }
            NodeValue::Link(node_link) => {
                if !(node_link.url.starts_with("http://") || node_link.url.starts_with("https://"))
                {
                    if let Ok(local_link) = LocalLink::from_str(
                        &node_link.url,
                        PathBuf::from(source.as_str()).parent().unwrap(),
                    ) {
                        local_links.push(local_link);
                    } else {
                        errors.push(format!("Failed to parse local link: {}", node_link.url));
                    }
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
                front_matter,
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
            ConfluencePage::version_message_prefix(),
            self.source.replace('\\', "/"), // needs to be platform independent
            self.checksum
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::confluence_page::ConfluencePage;
    use crate::error::TestResult;
    use crate::link_generator::LinkGenerator;
    use crate::markdown_page::{FrontMatter, LocalLink, MarkdownPage};
    use crate::responses::Version;
    use crate::template_renderer::TemplateRenderer;

    fn page_from_str<'a>(
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

        let content = page.to_html_string(&LinkGenerator::default())?;

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
                path: link_filename,
                anchor: Some(String::from("some-anchor"))
            }]
        );

        Ok(())
    }

    fn dummy_confluence_page(title: &str, id: &str) -> ConfluencePage {
        ConfluencePage {
            id: id.to_string(),
            title: title.to_string(),
            parent_id: None,
            version: Version {
                message: String::default(),
                number: 1,
            },
            path: None, // "foo.md".to_string(),
        }
    }

    #[test]
    fn it_uses_the_title_when_linktext_is_empty() -> TestResult {
        let link_filename = PathBuf::from("hello-world.md");
        let link_text = String::from("");
        let link_url = String::from("https://my.atlassian.net/wiki/spaces/TEAM/pages/47");
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

        let mut link_generator = LinkGenerator::new("my.atlassian.net", "TEAM");

        link_generator.register_markdown_page(&page)?;
        link_generator.register_markdown_page(&linked_page)?;
        link_generator.register_confluence_page(&dummy_confluence_page("A Linked Page", "47"));

        let content = page.to_html_string(&link_generator)?;
        println!("actual {:#?}", content);
        let expected = format!("<a href=\"{}\">{}</a>", link_url, "A Linked Page");
        println!("expect {:#?}", expected);
        assert!(content.contains(expected.as_str()));

        Ok(())
    }

    #[test]
    fn it_translates_file_links() -> TestResult {
        let link_filename = PathBuf::from("hello-world.md");
        let link_text = String::from("Link text");
        let link_url = String::from("https://my.atlassian.net/wiki/spaces/TEAM/pages/47");
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

        let mut link_generator = LinkGenerator::new("my.atlassian.net", "TEAM");

        link_generator.register_markdown_page(&page)?;
        link_generator.register_markdown_page(&linked_page)?;
        link_generator.register_confluence_page(&dummy_confluence_page("A Linked Page", "47"));

        let content = page.to_html_string(&link_generator)?;
        println!("actual {:#?}", content);
        let expected = format!("<a href=\"{}\">{}</a>", link_url, link_text);
        println!("expect {:#?}", expected);
        assert!(content.contains(expected.as_str()));

        Ok(())
    }

    #[test]
    fn it_renders_local_file_as_attached_image() -> TestResult {
        let content = "# My Page Title\n\nMy page content: ![myimage](myimage.png)";
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", content, &arena)?;

        assert_eq!(page.attachments.len(), 1);

        let content = page.to_html_string(&LinkGenerator::default())?;

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
        let html_content = page.to_html_string(&LinkGenerator::default())?;

        println!("Got content: {:#}", html_content);

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
        let html_content = page.to_html_string(&LinkGenerator::default())?;

        println!("Got content: {:#}", html_content);

        assert!(
            html_content.contains(format!(r#"<a href="{}">example</a>"#, external_url).as_str())
        );

        Ok(())
    }

    #[test]
    fn it_renders_templates() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{filename}}";
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), "<p>page.md</p>");

        Ok(())
    }

    #[test]
    fn it_renders_predefined_functions() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{hello_world()}}";
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), "<p><em>hello world!</em></p>");

        Ok(())
    }

    #[test]
    fn it_renders_builtins() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{hello_world(name=\"world!\")}}";

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default())?;

        assert_eq!(rendered_page.content.trim(), "<p><em>hello world!</em></p>");

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

        assert_eq!(
            page.front_matter,
            Some(FrontMatter {
                labels: vec!["foo".to_string(), "bar".to_string()]
            })
        );

        Ok(())
    }
}
