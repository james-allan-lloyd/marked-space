use std::collections::HashMap;

use anyhow::bail;
use saphyr::Yaml;
use tera::{self, Tera, Value};

use crate::builtins::add_builtins;
use crate::confluence_client::ConfluenceClient;
use crate::error::Result;
use crate::frontmatter::FrontMatter;
use crate::imports::generate_import_lines;
use crate::markdown_space::MarkdownSpace;
use crate::mentions::CachedMentions;

pub struct TemplateRenderer {
    tera: Tera,
    space_key: String,
}

fn make_metadata_lookup(metadata: Yaml) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let mut current_yml = &metadata;
            if let Some(path) = args.get("path") {
                for arg in path.as_str().unwrap().split(".") {
                    current_yml = &current_yml[arg];
                }
                if let Some(yaml_str) = current_yml.as_str() {
                    Ok(Value::from(yaml_str))
                } else {
                    Ok(Value::Null)
                }
            } else {
                Err("Missing parameter 'path'".into())
            }
        },
    )
}

// Required method
impl TemplateRenderer {
    pub fn new(space: &MarkdownSpace, client: &ConfluenceClient) -> Result<TemplateRenderer> {
        let space_key = space.key.clone();
        let mut tera = Tera::new(space.dir.join("**/*.md").into_os_string().to_str().unwrap())?;

        add_builtins(&mut tera)?;
        tera.register_function("mention", CachedMentions::new(client.clone()));

        Ok(TemplateRenderer { tera, space_key })
    }

    #[cfg(test)]
    pub fn default() -> Result<TemplateRenderer> {
        let mut tera = Tera::default();
        let space_key = String::from("SPACE");
        add_builtins(&mut tera)?;

        Ok(TemplateRenderer { tera, space_key })
    }

    #[cfg(test)]
    pub fn default_with_client(client: &ConfluenceClient) -> Result<TemplateRenderer> {
        use crate::mentions::CachedMentions;

        let mut tera = Tera::default();
        let space_key = String::from("SPACE");
        add_builtins(&mut tera)?;

        tera.register_function("mention", CachedMentions::new(client.clone()));

        Ok(TemplateRenderer { tera, space_key })
    }

    pub fn render_template_str(
        &mut self,
        source: &str,
        content: &str,
        fm: &FrontMatter,
    ) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("filename", &source);
        context.insert("default_space_key", &self.space_key);
        self.tera
            .register_function("metadata", make_metadata_lookup(fm.metadata.clone()));

        for import in fm.imports.iter() {
            if !self
                .tera
                .get_template_names()
                .any(|x| *x == String::from("_tera/") + import)
            {
                bail!(
                    "Import '{}' does not exist under the _tera directory",
                    import
                );
            }
        }

        let import_text = generate_import_lines(fm) + content;

        Ok(self.tera.render_str(&import_text, &context)?)
    }

    #[cfg(test)]
    pub fn add_raw_template(
        &mut self,
        template_name: &str,
        macro_str: &str,
    ) -> std::result::Result<(), tera::Error> {
        self.tera.add_raw_template(template_name, macro_str)
    }
}

#[cfg(test)]
mod test {
    use saphyr::Yaml;

    use crate::{error::TestResult, frontmatter::FrontMatter};

    use super::TemplateRenderer;

    #[test]
    fn it_puts_original_filename_in_message() -> TestResult {
        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.render_template_str(
            "test.md",
            "{{ func_does_not_exist() }}",
            &FrontMatter::default(),
        );
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.unwrap_err()),
            "Failed to render '__tera_one_off': Function 'func_does_not_exist' not found"
        );
        Ok(())
    }

    #[test]
    fn it_handles_different_metadata_across_files() -> TestResult {
        let fm1 = FrontMatter {
            metadata: Yaml::load_from_str("key: value").unwrap()[0].clone(),
            ..Default::default()
        };

        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.render_template_str(
            "test.md",
            "{{ metadata(path=\"key\") }}",
            &fm1,
        )?;

        assert_eq!(result, "value");

        let fm2 = FrontMatter {
            metadata: Yaml::load_from_str("other_key: other_value").unwrap()[0].clone(),
            ..Default::default()
        };

        let result2 = template_renderer.render_template_str(
            "test2.md",
            "{{ metadata(path=\"other_key\") }}",
            &fm2,
        )?;

        assert_eq!(result2, "other_value");

        Ok(())
    }
}
