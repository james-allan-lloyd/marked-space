use std::path::PathBuf;

use comrak::{nodes::AstNode, Arena};

use crate::{
    confluence_page::{self, ConfluenceNode, ConfluencePageData},
    error::Result,
    link_generator::LinkGenerator,
    markdown_page::{MarkdownPage, RenderedPage},
    responses::{self, ContentStatus},
};

pub fn markdown_page_from_str<'a>(
    filename: &str,
    content: &str,
    arena: &'a Arena<AstNode<'a>>,
) -> crate::error::Result<MarkdownPage<'a>> {
    use std::path::PathBuf;

    use crate::template_renderer::TemplateRenderer;

    MarkdownPage::from_str(
        &PathBuf::from(filename),
        content,
        arena,
        filename.to_string(),
        &mut TemplateRenderer::default()?,
    )
}

pub fn test_render(markdown_content: &str) -> Result<RenderedPage> {
    let arena = Arena::<AstNode>::new();
    let page = markdown_page_from_str("page.md", markdown_content, &arena)?;
    page.render(&LinkGenerator::default_test())
}

pub fn register_mark_and_conf_page<'a>(
    page_id: &str,
    link_generator: &mut LinkGenerator,
    markdown_page: crate::markdown_page::MarkdownPage<'a>,
) -> Result<crate::markdown_page::MarkdownPage<'a>> {
    link_generator.register_markdown_page(&markdown_page)?;
    link_generator.register_confluence_node(&ConfluenceNode {
        id: page_id.into(),
        title: markdown_page.title.clone(),
        parent_id: Some("99".into()),
        data: <confluence_page::ConfluenceNodeType>::from(ConfluencePageData {
            version: responses::Version {
                number: 1,
                message: ConfluencePageData::version_message_prefix().into(),
            },
            path: Some(PathBuf::from(&markdown_page.source)),
            status: ContentStatus::Current,
        }),
    });
    Ok(markdown_page)
}
