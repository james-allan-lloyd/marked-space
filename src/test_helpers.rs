use comrak::{nodes::AstNode, Arena};

use crate::markdown_page::MarkdownPage;

#[cfg(test)]
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
