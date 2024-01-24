use std::collections::HashMap;

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

fn labellist(
    args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    let label = args
        .get(&"labels".to_string())
        .ok_or("Missing required argument 'labels'")?;

    let parameter = match label {
        serde_json::Value::String(s) => format!("label = \"{}\"", s),
        serde_json::Value::Array(a) => {
            if !a.is_empty() {
                format!(
                    "label in ({})",
                    a.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join(",")
                )
            } else {
                Err(tera::Error::msg("labels needs to be a non-empty array"))?
            }
        }
        _ => Err(tera::Error::msg("labels needs to be a string or array"))?,
    };
    Ok(
        serde_json::to_value(
format!(r#"<ac:structured-macro ac:name="contentbylabel" ac:schema-version="4" data-layout="default" ac:macro-id="808ece5f-14fd-4c2d-853a-bf87e0696e48">
    <ac:parameter ac:name="cql">{} and space = currentSpace()</ac:parameter>
</ac:structured-macro>"#, parameter) 
        ).unwrap()
    )
}

impl TemplateRenderer {
    pub fn new(space: &MarkdownSpace) -> Result<TemplateRenderer> {
        let mut tera = Tera::new(space.dir.join("**/*.md").into_os_string().to_str().unwrap())?;

        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    fn add_builtins(tera: &mut Tera) -> Result<()> {
        tera.register_function("hello_world", hello_world);
        tera.register_function("toc", toc);
        tera.register_function("children", children);
        tera.register_function("labellist", labellist);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn default() -> Result<TemplateRenderer> {
        let mut tera = Tera::default();
        Self::add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera })
    }

    pub fn render_template(&mut self, source: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("filename", &source);

        let result = self.tera.render(&source.replace('\\', "/"), &context);

        Ok(result?)
    }

    pub fn expand_html_str(&mut self, source: &str, content: &str) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("filename", &source);

        let result = self.tera.render_str(content, &context);

        Ok(result?)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::error::TestResult;

    use super::{labellist, TemplateRenderer};

    #[test]
    fn it_puts_original_filename_in_message() -> TestResult {
        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.expand_html_str("test.md", "{{ func_does_not_exist() }}");
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.unwrap_err()),
            "Failed to render '__tera_one_off': Function 'func_does_not_exist' not found"
        );
        Ok(())
    }

    #[test]
    fn labellist_allows_multiple_labels() -> TestResult {
        let args = HashMap::from([("labels".to_string(), serde_json::Value::from("foo"))]);
        let result = labellist(&args);

        assert!(result.is_ok());

        let args = HashMap::from([(
            "labels".to_string(),
            serde_json::Value::from(vec!["foo", "bar"]),
        )]);
        let result = labellist(&args);

        assert!(result.is_ok());
        let s = result.unwrap();
        println!("s: {:#}", s);
        assert!(s.to_string().contains(r#"label in (\"foo\",\"bar\")"#));

        Ok(())
    }
}
