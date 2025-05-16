use std::collections::HashMap;

use crate::error::Result;
use tera::Tera;

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

const PROPERTIES_TABLE: &str = r###"{% macro properties(metadata) -%}
<ac:structured-macro ac:name="details" ac:schema-version="1" data-layout="default" ac:local-id="779bc5f9-b8c3-41df-bccc-1840efc20a80" ac:macro-id="4008e080-6218-49a8-82f8-1387005d53d2"><ac:rich-text-body >
<table><tbody>
{%- for metadata_key in metadata -%}
<tr><th>{{ metadata_key|title }}</th><td>{{ metadata(path=metadata_key) }}</td></tr>
{%- endfor -%}
</tbody></table></ac:rich-text-body></ac:structured-macro>

{%- endmacro %}

{% macro properties_report(space='', label) -%}
<ac:structured-macro ac:name="detailssummary" ac:schema-version="2">
    <ac:parameter ac:name="firstcolumn">Title</ac:parameter>
    <ac:parameter ac:name="sortBy">Title</ac:parameter>
    <ac:parameter ac:name="cql">space = {% if space.len %}{{space}}{% else %}{{ default_space_key }}{%endif%} and label = "{{ label }}"</ac:parameter>
</ac:structured-macro>
{%- endmacro %}
"###;

pub(crate) fn add_builtins(tera: &mut Tera) -> Result<()> {
    tera.register_function("hello_world", hello_world);
    tera.register_function("toc", toc);
    tera.register_function("children", children);
    tera.register_function("labellist", labellist);
    tera.add_raw_template("_tera/builtins", PROPERTIES_TABLE)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use comrak::{nodes::AstNode, Arena};
    use scraper::{Html, Selector};

    use crate::{
        builtins::labellist,
        error::Result,
        error::TestResult,
        link_generator::LinkGenerator,
        markdown_page::{page_from_str, RenderedPage},
    };

    fn test_render(markdown_content: &str) -> Result<RenderedPage> {
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", markdown_content, &arena)?;
        page.render(&LinkGenerator::default())
    }

    fn extract_properties_table(parsed_html: Html) -> Vec<(String, String)> {
        let parsed_table: Vec<(String, String)> = parsed_html
            .select(&Selector::parse("tr").unwrap())
            .map(|row| {
                let header = row
                    .select(&Selector::parse("th").unwrap())
                    .next()
                    .unwrap()
                    .text()
                    .collect();
                let value = row
                    .select(&Selector::parse("td").unwrap())
                    .next()
                    .unwrap()
                    .text()
                    .collect();
                println!("{:?}", (&header, &value));
                (header, value)
            })
            .collect();
        parsed_table
    }

    #[test]
    fn it_renders_predefined_functions() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{hello_world()}}";
        let page = page_from_str("page.md", markdown_content, &arena)?;

        let rendered_page = page.render(&LinkGenerator::default_test())?;

        assert_eq!(rendered_page.content.trim(), "<p><em>hello world!</em></p>");
        Ok(())
    }

    #[test]
    fn it_renders_builtins() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let markdown_content = "# compulsory title\n{{hello_world(name=\"world!\")}}";

        let page = page_from_str("page.md", markdown_content, &arena)?;
        let rendered_page = page.render(&LinkGenerator::default_test())?;

        assert_eq!(rendered_page.content.trim(), "<p><em>hello world!</em></p>");

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

    #[test]
    fn properties_report_defaults_to_current_space() -> TestResult {
        let rendered_page =
            test_render("# compulsory title\n{{ builtins::properties_report(label=\"foo\") }}")?;

        assert_eq!(rendered_page.content.trim(), "<p><ac:structured-macro ac:name=\"detailssummary\" ac:schema-version=\"2\"> <ac:parameter ac:name=\"firstcolumn\">Title</ac:parameter> <ac:parameter ac:name=\"sortBy\">Title</ac:parameter> <ac:parameter ac:name=\"cql\">space = SPACE and label = \"foo\"</ac:parameter> </ac:structured-macro></p>");

        Ok(())
    }

    #[test]
    fn properties_renders_from_metadata() -> TestResult {
        let rendered_page = test_render(
            r###"---
metadata:
    Owner: James
    Status: Complete
---
# compulsory title

{{ builtins::properties(metadata=["Owner", "Status"]) }}
"###,
        )?;

        // assert_eq!(rendered_page.content.trim(), "");
        println!("{}", rendered_page.content.trim());

        let parsed_html = Html::parse_fragment(rendered_page.content.trim());
        let parsed_table = extract_properties_table(parsed_html);

        assert_eq!(
            parsed_table,
            vec![
                (String::from("Owner"), String::from("James")),
                (String::from("Status"), String::from("Complete"))
            ]
        );

        Ok(())
    }

    #[test]
    fn properties_renders_from_metadata_capitalize_keys() -> TestResult {
        let rendered_page = test_render(
            r###"---
metadata:
    owner: James
    status: Complete
---
# compulsory title

{{ builtins::properties(metadata=["owner", "status"]) }}
"###,
        )?;

        // assert_eq!(rendered_page.content.trim(), "");
        println!("{}", rendered_page.content.trim());

        let parsed_html = Html::parse_fragment(rendered_page.content.trim());
        let parsed_table = extract_properties_table(parsed_html);

        assert_eq!(
            parsed_table,
            vec![
                (String::from("Owner"), String::from("James")),
                (String::from("Status"), String::from("Complete"))
            ]
        );

        Ok(())
    }

    #[test]
    fn properties_renders_from_metadata_passing_through_templates() -> TestResult {
        let rendered_page = test_render(
            r###"---
metadata:
    owner: James
    status: "{{ Self::status(value=Complete) }}"
---
# compulsory title
{% macro status(value) -%}
Status: {{value}}
{%- endmacro %}

{{ builtins::properties(metadata=["owner", "status"]) }}
"###,
        )?;

        // assert_eq!(rendered_page.content.trim(), "");
        println!("{}", rendered_page.content.trim());

        let parsed_html = Html::parse_fragment(rendered_page.content.trim());
        let parsed_table = extract_properties_table(parsed_html);

        assert_eq!(
            parsed_table,
            vec![
                (String::from("Owner"), String::from("James")),
                (String::from("Status"), String::from("Status: Complete"))
            ]
        );

        Ok(())
    }
}
