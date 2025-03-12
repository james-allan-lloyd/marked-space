use std::path::Path;

use crate::frontmatter::FrontMatter;

pub fn generate_import_lines(fm: &FrontMatter) -> String {
    fn generate_import_line(import: &String) -> String {
        let import_path = Path::new(import);
        let parent = if let Some(parent_path) = import_path.parent() {
            let parent_str = parent_path.to_string_lossy().into_owned();
            if parent_str.is_empty() {
                parent_str
            } else {
                parent_str + "_"
            }
        } else {
            String::new()
        };
        let namespace = Path::new(&import).file_stem().unwrap();
        format!(
            "{{% import '_tera/{}' as {} %}}\n",
            import,
            parent + &namespace.to_string_lossy()
        )
    }

    fm.imports
        .iter()
        .map(generate_import_line)
        .collect::<Vec<String>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use crate::{error::TestResult, frontmatter::FrontMatter, template_renderer::TemplateRenderer};

    use super::generate_import_lines;

    static TEST_MACRO: &str = r##"
{% macro test() -%}
test passed
{%- endmacro test %}
"##;
    #[test]
    fn it_generates_import_lines() -> TestResult {
        let fm1 = FrontMatter {
            imports: vec![String::from("test.md")],
            ..Default::default()
        };

        let import_lines = generate_import_lines(&fm1);
        assert_eq!(import_lines, "{% import '_tera/test.md' as test %}\n");
        Ok(())
    }

    #[test]
    fn it_imports_macros_from_templates_specified_in_frontmatter() -> TestResult {
        let fm1 = FrontMatter {
            imports: vec![String::from("test.md")],
            ..Default::default()
        };

        let mut template_renderer = TemplateRenderer::default()?;

        template_renderer.add_raw_template("_tera/test.md", TEST_MACRO)?;

        let result =
            template_renderer.render_template_str("test.md", "{{ test::test() }}", &fm1)?;
        assert_eq!(result, "test passed");
        Ok(())
    }

    #[test]
    fn it_errors_if_the_import_does_not_exist() -> TestResult {
        let fm1 = FrontMatter {
            imports: vec![String::from("does_not_exist.md")],
            ..Default::default()
        };

        let mut template_renderer = TemplateRenderer::default()?;
        let result = template_renderer.render_template_str("test.md", "{{ test::test() }}", &fm1);
        assert!(result.is_err());
        assert_eq!(
            format!("{:?}", result),
            "Err(Import 'does_not_exist.md' does not exist under the _tera directory)"
        );
        Ok(())
    }

    #[test]
    fn it_uses_the_basename_of_the_file_as_the_namespace() -> TestResult {
        let fm1 = FrontMatter {
            imports: vec![String::from("adr.md")],
            ..Default::default()
        };

        let mut template_renderer = TemplateRenderer::default()?;

        template_renderer.add_raw_template("_tera/adr.md", TEST_MACRO)?;

        let result = template_renderer.render_template_str("test.md", "{{ adr::test() }}", &fm1);
        assert!(result.is_ok());
        assert_eq!(result?, "test passed");

        Ok(())
    }

    #[test]
    fn it_handles_imports_from_subdirs() -> TestResult {
        let fm1 = FrontMatter {
            imports: vec![String::from("subdir/test.md")],
            ..Default::default()
        };

        let mut template_renderer = TemplateRenderer::default()?;

        template_renderer.add_raw_template("_tera/subdir/test.md", TEST_MACRO)?;
        let result =
            template_renderer.render_template_str("test.md", "{{ subdir_test::test() }}", &fm1);
        assert_eq!(result?, "test passed");
        Ok(())
    }
}
