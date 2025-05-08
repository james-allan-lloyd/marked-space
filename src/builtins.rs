use std::collections::HashMap;

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

fn properties_report(
    args: &HashMap<String, tera::Value>,
    default_space_key: &str,
) -> std::result::Result<serde_json::Value, tera::Error> {
    // We already know the space
    let space = args
        .get(&"space".to_string())
        .map_or(String::from(default_space_key), |s| s.to_string());

    let label = args
        .get(&"label".to_string())
        .ok_or("Missing required argument 'label'")?;

    Ok(serde_json::to_value(format!(
        r#"<ac:structured-macro ac:name="detailssummary" ac:schema-version="2">
        <ac:parameter ac:name="firstcolumn">Title</ac:parameter>
        <ac:parameter ac:name="sortBy">Title</ac:parameter>
        <ac:parameter ac:name="cql">space = {} and label = {}</ac:parameter>
    </ac:structured-macro>"#,
        space, label
    ))
    .unwrap())
}

fn properties(
    _args: &HashMap<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, tera::Error> {
    // TODO: Read these from the properties section from frontmatter
    let status = "New";
    let owner = "John Doe";

    let structure = format!(
        "\\<ac:structured-macro ac:name=\"details\" ac:schema-version=\"1\"\\>\
        \\<ac:rich-text-body\\>\
            <table>\
                <tbody>\
                    <tr><th><strong>Status</strong></th><td>{}</td></tr>\
                    <tr><th><strong>Owner</strong></th><td>{}</td></tr>\
                </tbody>\
            </table>\
        \\</ac:rich-text-body\\>\
    \\</ac:structured-macro\\>",
        status, owner
    );

    Ok(serde_json::to_value(structure).unwrap())
}

pub(crate) fn add_builtins(tera: &mut Tera, default_space_key: String) {
    tera.register_function("hello_world", hello_world);
    tera.register_function("toc", toc);
    tera.register_function("children", children);
    tera.register_function("labellist", labellist);
    tera.register_function(
        "properties_report",
        Box::new(move |args: &HashMap<String, tera::Value>| {
            properties_report(args, &default_space_key)
        }),
    );
    tera.register_function("properties", properties);
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use comrak::{nodes::AstNode, Arena};

    use crate::{
        builtins::labellist,
        error::Result,
        error::TestResult,
        link_generator::LinkGenerator,
        markdown_page::{page_from_str, RenderedPage},
    };

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

    fn test_render(markdown_content: &str) -> Result<RenderedPage> {
        let arena = Arena::<AstNode>::new();
        let page = page_from_str("page.md", markdown_content, &arena)?;
        page.render(&LinkGenerator::default())
    }

    #[test]
    fn properties_report_defaults_to_current_space() -> TestResult {
        let rendered_page =
            test_render("# compulsory title\n{{ properties_report(label=\"foo\") }}")?;

        assert_eq!(rendered_page.content.trim(), "<p><ac:structured-macro ac:name=\"detailssummary\" ac:schema-version=\"2\"> <ac:parameter ac:name=\"firstcolumn\">Title</ac:parameter> <ac:parameter ac:name=\"sortBy\">Title</ac:parameter> <ac:parameter ac:name=\"cql\">space = SPACE and label = \"foo\"</ac:parameter> </ac:structured-macro></p>");

        Ok(())
    }
}
