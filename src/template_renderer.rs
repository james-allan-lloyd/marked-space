use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use tera::{self, Tera};

use crate::error::Result;
use crate::markdown_page::MarkdownPage;
use crate::markdown_space::MarkdownSpace;

pub struct TemplateRenderer {
    tera: Tera,
}

fn hello_world(
    _args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    Ok(serde_json::to_value("<em>hello world!</em>").unwrap())
}

impl TemplateRenderer {
    pub fn new(space: &MarkdownSpace) -> Result<TemplateRenderer> {
        let mut tera = Tera::new(
            space
                .dir
                .join("_templates/**/*.html")
                .into_os_string()
                .to_str()
                .unwrap(),
        )?;

        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    fn add_builtins(tera: &mut Tera) -> Result<()> {
        tera.register_function("hello_world", hello_world);
        tera.add_raw_template(
            "macros.md",
            r##"
            {% macro hello(name) -%}<em>hello {{name}}</em>{%- endmacro hello %}

            {% macro toc() -%}
            <ac:structured-macro ac:name="toc" ac:schema-version="1" data-layout="default" ac:macro-id="334277ff-40b1-45ec-b5c7-ba6091fd0df3"><ac:parameter ac:name="minLevel">1</ac:parameter><ac:parameter ac:name="maxLevel">6</ac:parameter><ac:parameter ac:name="include" /><ac:parameter ac:name="outline">false</ac:parameter><ac:parameter ac:name="indent" /><ac:parameter ac:name="exclude" /><ac:parameter ac:name="type">list</ac:parameter><ac:parameter ac:name="class" /><ac:parameter ac:name="printable">false</ac:parameter></ac:structured-macro>
            {%- endmacro toc %}

            {% macro children() -%}
            <ac:structured-macro ac:name="children" ac:schema-version="2" data-layout="default" ac:macro-id="4172775450124db364aa2f7e7faf4cb3" />
            {%- endmacro chidlren %}
            "##,
        )?;
        Ok(())
    }

    pub fn default() -> Result<TemplateRenderer> {
        let mut tera = Tera::default();
        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    pub fn expand_html(&mut self, page: &MarkdownPage, content: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("filename", &page.source);
        context.insert("headings", &page.headings);
        let path = PathBuf::from(&page.source).with_extension("html");

        let template_name = path.to_str().unwrap();
        let content = String::from("{% import \"macros.md\" as macros %}") + content;

        self.tera
            .add_raw_template(template_name, content.as_str())?;

        let content = self.tera.render(template_name, &context).context(format!(
            "Failure expanding templates in '{}' after HTML rendering",
            page.source
        ))?;
        self.tera.templates.remove(template_name);
        Ok(content)
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use comrak::{nodes::AstNode, Arena};

    use crate::{error::TestResult, markdown_page::MarkdownPage};

    use super::TemplateRenderer;

    #[test]
    fn it_puts_original_filename_in_message() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let path = "test.md";
        let page = MarkdownPage::from_str(
            &PathBuf::from(path).as_path(),
            "# Compulsory Header\nsometext",
            &arena,
            path.into(),
        )?;

        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.expand_html(&page, "{{ func_does_not_exist() }}");
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.unwrap_err()),
            "Failure expanding templates in 'test.md' after HTML rendering: Failed to render 'test.html': Function 'func_does_not_exist' not found"
        );
        Ok(())
    }
}
