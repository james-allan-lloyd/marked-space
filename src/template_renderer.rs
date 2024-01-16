use std::collections::HashMap;

use anyhow::Context;
use tera::{self, Tera};

use crate::error::Result;
use crate::markdown_space::MarkdownSpace;

pub struct TemplateRenderer {
    tera: Tera,
}

fn hello_world(
    _args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    Ok(serde_json::to_value("<em>hello world!</em>").unwrap())
}

fn toc(
    _args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    Ok(
        serde_json::to_value(
        r#"<ac:structured-macro ac:name="toc" ac:schema-version="1" data-layout="default" ac:macro-id="334277ff-40b1-45ec-b5c7-ba6091fd0df3">
        <ac:parameter ac:name="minLevel">1</ac:parameter>
        <ac:parameter ac:name="maxLevel">6</ac:parameter>
        <ac:parameter ac:name="include" />
        <ac:parameter ac:name="outline">false</ac:parameter>
        <ac:parameter ac:name="indent" />
        <ac:parameter ac:name="exclude" />
        <ac:parameter ac:name="type">list</ac:parameter>
        <ac:parameter ac:name="class" />
        <ac:parameter ac:name="printable">false</ac:parameter>
    </ac:structured-macro>"#).unwrap()
    )
}

fn children(
    _args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    Ok(
        serde_json::to_value(
        r#"<ac:structured-macro ac:name="children" ac:schema-version="2" data-layout="default" ac:macro-id="4172775450124db364aa2f7e7faf4cb3" />"#
        ).unwrap()
    )
}

impl TemplateRenderer {
    pub fn new(space: &MarkdownSpace) -> Result<TemplateRenderer> {
        let mut tera = Tera::new(
            space
                .dir
                .join("_tera/**/*")
                .into_os_string()
                .to_str()
                .unwrap(),
        )?;

        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    fn add_builtins(tera: &mut Tera) -> Result<()> {
        tera.register_function("hello_world", hello_world);
        tera.register_function("toc", toc);
        tera.register_function("children", children);
        // tera.add_raw_template(
        //     "macros.md",
        //     r##"
        //     {% macro hello(name) -%}<em>hello {{name}}</em>{%- endmacro hello %}

        //     {% macro toc() -%}
        //     <ac:structured-macro ac:name="toc" ac:schema-version="1" data-layout="default" ac:macro-id="334277ff-40b1-45ec-b5c7-ba6091fd0df3"><ac:parameter ac:name="minLevel">1</ac:parameter><ac:parameter ac:name="maxLevel">6</ac:parameter><ac:parameter ac:name="include" /><ac:parameter ac:name="outline">false</ac:parameter><ac:parameter ac:name="indent" /><ac:parameter ac:name="exclude" /><ac:parameter ac:name="type">list</ac:parameter><ac:parameter ac:name="class" /><ac:parameter ac:name="printable">false</ac:parameter></ac:structured-macro>
        //     {%- endmacro toc %}

        //     {% macro children() -%}
        //     <ac:structured-macro ac:name="children" ac:schema-version="2" data-layout="default" ac:macro-id="4172775450124db364aa2f7e7faf4cb3" />
        //     {%- endmacro chidlren %}
        //     "##,
        // )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn default() -> Result<TemplateRenderer> {
        let mut tera = Tera::default();
        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    pub fn expand_html(&mut self, source: &str, content: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("filename", &source);
        self.tera
            .add_raw_template(source, content)
            .context(format!("Failed to parse template '{}'", source))?;

        let result = self.tera.render(source, &context);
        self.tera.templates.remove(source);

        Ok(result?)
    }
}

#[cfg(test)]
mod test {
    use crate::error::TestResult;

    use super::TemplateRenderer;

    #[test]
    fn it_puts_original_filename_in_message() -> TestResult {
        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.expand_html("test.md", "{{ func_does_not_exist() }}");
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.unwrap_err()),
            "Failed to render 'test.md': Function 'func_does_not_exist' not found"
        );
        Ok(())
    }
}
