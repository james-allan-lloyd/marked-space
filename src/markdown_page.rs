use std::{
    fs,
    ops::DerefMut,
    path::{Path, PathBuf},
};

use comrak::{
    format_html,
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};

use crate::{error::ConfluenceError, Result};

pub struct MarkdownPage<'a> {
    pub title: String,
    pub source: String,
    // arena: Arena<AstNode<'a>>,
    // nodes: Vec<AstNode<'a>>,
    // root: Option<AstNode<'a>>,
}

fn parse_new<'a>(markdown_page: &Path) -> Result<MarkdownPage<'a>> {
    let source = markdown_page.display().to_string();
    let content = match fs::read_to_string(markdown_page) {
        Ok(c) => c,
        Err(err) => {
            return Err(ConfluenceError::new(format!(
                "Failed to read file {}: {}",
                markdown_page.display(),
                err.to_string()
            )))
        }
    };

    let arena = Arena::new();

    {
        // let borrow: &'a Arena<AstNode<'a>> = &arena;
        parse(&arena, content);
    }

    let nodes = arena.into_vec();

    // fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
    // where
    //     F: FnMut(&'a AstNode<'a>),
    // {
    //     f(node);
    //     for c in node.children() {
    //         iter_nodes(c, f);
    //     }
    // }

    // let mut first_heading: Option<String> = None;

    // iter_nodes(root, &mut |node| match &mut node.data.borrow_mut().value {
    //     NodeValue::Heading(_heading) => {
    //         if first_heading.is_none() {
    //             let mut heading_text = String::default();
    //             // TODO: this is a double iteration of children
    //             for c in node.children() {
    //                 iter_nodes(c, &mut |child| match &mut child.data.borrow_mut().value {
    //                     NodeValue::Text(text) => {
    //                         println!("heading text {}", text);
    //                         heading_text += text
    //                     }
    //                     _ => (),
    //                 });
    //             }
    //             first_heading = Some(heading_text);
    //         }
    //     }
    //     &mut NodeValue::Text(ref mut text) => {
    //         let orig = std::mem::replace(text, String::default());
    //         *text = orig.clone().replace("my", "your");
    //     }
    //     _ => (),
    // });

    // println!("{:#?}", first_heading);

    // if first_heading.is_none() {
    //     return Err(ConfluenceError::new("Missing first heading"));
    // }

    Ok(MarkdownPage {
        title: String::default(),
        source,
        // arena,
        // nodes,
    })
}

fn parse<'a>(
    arena: &'a Arena<comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>>>,
    content: String,
) -> &comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>> {
    parse_document(arena, content.as_str(), &Options::default())
}

impl<'a> MarkdownPage<'a> {
    // pub fn new() -> Result<Self> {
    //     Ok(MarkdownPage {
    //         title: String::default(),
    //         source: markdown_page.display().to_string(),
    //         arena: Arena::new(),
    //         // nodes: Vec::default(),
    //         // root: None,
    //     })
    // }

    pub fn parse<'b>(markdown_page: &'b Path) -> Result<MarkdownPage<'b>> {
        parse_new(markdown_page)
    }

    pub fn to_html_string(&self) -> Result<String> {
        Ok(String::from("Bar"))
        // let mut html = vec![];
        // format_html(&self.root, &Options::default(), &mut html).unwrap();

        // match String::from_utf8(html) {
        //     Ok(content) => Ok(content),
        //     Err(_err) => Err(ConfluenceError::new("Failed to convert to utf8")),
        // }
    }
}
