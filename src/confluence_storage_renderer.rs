//! A renderer for the CommonMark AST to the Confluence Storage format.
//! This was adapted from https://github.com/kivikakk/comrak/blob/main/src/html.rs
use comrak::nodes::{
    AstNode, ListType, NodeCode, NodeFootnoteDefinition, NodeTable, NodeValue, TableAlignment,
};
use comrak::Options;
use std::cell::Cell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str;

use once_cell::sync::Lazy;

use crate::alerts::{render_basic_alert, render_expand};
use crate::attachments::{render_link_enter, render_link_leave};
use crate::link_generator::LinkGenerator;

#[rustfmt::skip]
const CMARK_CTYPE_CLASS: [u8; 256] = [
    /*      0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f */
    /* 0 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0,
    /* 1 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* 2 */ 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    /* 3 */ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2,
    /* 4 */ 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    /* 5 */ 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 2, 2, 2, 2,
    /* 6 */ 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    /* 7 */ 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 2, 2, 2, 0,
    /* 8 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* 9 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* a */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* b */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* c */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* d */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* e */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    /* f */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn isspace(ch: u8) -> bool {
    CMARK_CTYPE_CLASS[ch as usize] == 1
}

/// Formats an AST as HTML, modified by the given options. Accepts custom plugins.
pub fn render_confluence_storage<'a>(
    root: &'a AstNode<'a>,
    options: &Options,
    output: &mut dyn Write,
    link_generator: &LinkGenerator,
    source: &Path,
) -> io::Result<()> {
    let mut writer = WriteWithLast::from_write(output);
    let mut f = ConfluenceStorageRenderer::new(options, &mut writer, link_generator, source);
    f.format(root, false)?;
    if f.footnote_ix > 0 {
        f.output.write_all(b"</ol>\n</section>\n")?;
    }
    Ok(())
}

pub struct WriteWithLast<'w> {
    output: &'w mut dyn Write,
    last_was_lf: Cell<bool>,
}

impl<'w> WriteWithLast<'w> {
    pub fn from_write(output: &'w mut dyn Write) -> Self {
        Self {
            output,
            last_was_lf: Cell::new(true),
        }
    }
}

impl Write for WriteWithLast<'_> {
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let l = buf.len();
        if l > 0 {
            self.last_was_lf.set(buf[l - 1] == 10);
        }
        self.output.write(buf)
    }
}

pub struct ConfluenceStorageRenderer<'o> {
    pub output: &'o mut WriteWithLast<'o>,
    options: &'o Options<'o>,
    footnote_ix: u32,
    written_footnote_ix: u32,
    link_generator: &'o LinkGenerator,
    next_task_id: u32,
    pub source: PathBuf,
}

#[rustfmt::skip]
const NEEDS_ESCAPED : [bool; 256] = [
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, true,  false, false, false, true,  false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, true, false, true, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
];

fn tagfilter(literal: &[u8]) -> bool {
    static TAGFILTER_BLACKLIST: [&str; 9] = [
        "title",
        "textarea",
        "style",
        "xmp",
        "iframe",
        "noembed",
        "noframes",
        "script",
        "plaintext",
    ];

    if literal.len() < 3 || literal[0] != b'<' {
        return false;
    }

    let mut i = 1;
    if literal[i] == b'/' {
        i += 1;
    }

    let lc = unsafe { String::from_utf8_unchecked(literal[i..].to_vec()) }.to_lowercase();
    for t in TAGFILTER_BLACKLIST.iter() {
        if lc.starts_with(t) {
            let j = i + t.len();
            return isspace(literal[j])
                || literal[j] == b'>'
                || (literal[j] == b'/' && literal.len() >= j + 2 && literal[j + 1] == b'>');
        }
    }

    false
}

fn tagfilter_block(input: &[u8], o: &mut dyn Write) -> io::Result<()> {
    let size = input.len();
    let mut i = 0;

    while i < size {
        let org = i;
        while i < size && input[i] != b'<' {
            i += 1;
        }

        if i > org {
            o.write_all(&input[org..i])?;
        }

        if i >= size {
            break;
        }

        if tagfilter(&input[i..]) {
            o.write_all(b"&lt;")?;
        } else {
            o.write_all(b"<")?;
        }

        i += 1;
    }

    Ok(())
}

/// Writes buffer to output, escaping anything that could be interpreted as an
/// HTML tag.
///
/// Namely:
///
/// * U+0022 QUOTATION MARK " is rendered as &quot;
/// * U+0026 AMPERSAND & is rendered as &amp;
/// * U+003C LESS-THAN SIGN < is rendered as &lt;
/// * U+003E GREATER-THAN SIGN > is rendered as &gt;
/// * Everything else is passed through unchanged.
///
/// Note that this is appropriate and sufficient for free text, but not for
/// URLs in attributes.  See escape_href.
pub fn escape(output: &mut dyn Write, buffer: &[u8]) -> io::Result<()> {
    let mut offset = 0;
    for (i, &byte) in buffer.iter().enumerate() {
        if NEEDS_ESCAPED[byte as usize] {
            let esc: &[u8] = match byte {
                b'"' => b"&quot;",
                b'&' => b"&amp;",
                b'<' => b"&lt;",
                b'>' => b"&gt;",
                _ => unreachable!(),
            };
            output.write_all(&buffer[offset..i])?;
            output.write_all(esc)?;
            offset = i + 1;
        }
    }
    output.write_all(&buffer[offset..])?;
    Ok(())
}

/// Writes buffer to output, escaping in a manner appropriate for URLs in HTML
/// attributes.
///
/// Namely:
///
/// * U+0026 AMPERSAND & is rendered as &amp;
/// * U+0027 APOSTROPHE ' is rendered as &#x27;
/// * Alphanumeric and a range of non-URL safe characters.
///
/// The inclusion of characters like "%" in those which are not escaped is
/// explained somewhat here:
///
/// https://github.com/github/cmark-gfm/blob/c32ef78bae851cb83b7ad52d0fbff880acdcd44a/src/houdini_href_e.c#L7-L31
///
/// In other words, if a CommonMark user enters:
///
/// ```markdown
/// [hi](https://ddg.gg/?q=a%20b)
/// ```
///
/// We assume they actually want the query string "?q=a%20b", a search for
/// the string "a b", rather than "?q=a%2520b", a search for the literal
/// string "a%20b".
pub fn escape_href(output: &mut dyn Write, buffer: &[u8]) -> io::Result<()> {
    static HREF_SAFE: Lazy<[bool; 256]> = Lazy::new(|| {
        let mut a = [false; 256];
        for &c in b"-_.+!*(),%#@?=;:/,+$~abcdefghijklmnopqrstuvwxyz".iter() {
            a[c as usize] = true;
        }
        for &c in b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".iter() {
            a[c as usize] = true;
        }
        a
    });

    let size = buffer.len();
    let mut i = 0;

    while i < size {
        let org = i;
        while i < size && HREF_SAFE[buffer[i] as usize] {
            i += 1;
        }

        if i > org {
            output.write_all(&buffer[org..i])?;
        }

        if i >= size {
            break;
        }

        match buffer[i] as char {
            '&' => {
                output.write_all(b"&amp;")?;
            }
            '\'' => {
                output.write_all(b"&#x27;")?;
            }
            _ => write!(output, "%{:02X}", buffer[i])?,
        }

        i += 1;
    }

    Ok(())
}

impl<'o> ConfluenceStorageRenderer<'o> {
    fn new(
        options: &'o Options,
        output: &'o mut WriteWithLast<'o>,
        link_generator: &'o LinkGenerator,
        source: &Path,
    ) -> Self {
        ConfluenceStorageRenderer {
            options,
            output,
            footnote_ix: 0,
            written_footnote_ix: 0,
            link_generator,
            next_task_id: 1,
            source: PathBuf::from(source),
        }
    }

    fn cr(&mut self) -> io::Result<()> {
        if !self.output.last_was_lf.get() {
            self.output.write_all(b"\n")?;
        }
        Ok(())
    }

    pub fn escape(&mut self, buffer: &[u8]) -> io::Result<()> {
        escape(&mut self.output, buffer)
    }

    fn escape_href(&mut self, buffer: &[u8]) -> io::Result<()> {
        escape_href(&mut self.output, buffer)
    }

    fn format<'a>(&mut self, node: &'a AstNode<'a>, plain: bool) -> io::Result<()> {
        // Traverse the AST iteratively using a work stack, with pre- and
        // post-child-traversal phases. During pre-order traversal render the
        // opening tags, then push the node back onto the stack for the
        // post-order traversal phase, then push the children in reverse order
        // onto the stack and begin rendering first child.
        enum Phase {
            Pre,
            Post,
        }
        let mut stack = vec![(node, plain, Phase::Pre)];

        while let Some((node, plain, phase)) = stack.pop() {
            match phase {
                Phase::Pre => {
                    let new_plain = if plain {
                        match node.data.borrow().value {
                            NodeValue::Text(ref literal)
                            | NodeValue::Code(NodeCode { ref literal, .. })
                            | NodeValue::HtmlInline(ref literal) => {
                                self.escape(literal.as_bytes())?;
                            }
                            NodeValue::LineBreak | NodeValue::SoftBreak => {
                                self.output.write_all(b" ")?;
                            }
                            _ => (),
                        }
                        plain
                    } else {
                        stack.push((node, false, Phase::Post));
                        self.format_node(node, true)?
                    };

                    for ch in node.reverse_children() {
                        stack.push((ch, new_plain, Phase::Pre));
                    }
                }
                Phase::Post => {
                    debug_assert!(!plain);
                    self.format_node(node, false)?;
                }
            }
        }

        Ok(())
    }

    fn format_node<'a>(&mut self, node: &'a AstNode<'a>, entering: bool) -> io::Result<bool> {
        match node.data.borrow().value {
            NodeValue::Document => (),
            NodeValue::FrontMatter(_) => (),
            NodeValue::BlockQuote => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<blockquote")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">\n")?;
                } else {
                    self.cr()?;
                    self.output.write_all(b"</blockquote>\n")?;
                }
            }
            NodeValue::List(ref nl) => {
                if entering {
                    self.cr()?;
                    if nl.list_type == ListType::Bullet {
                        if has_task_children(node) {
                            self.output.write_all(b"<ac:task-list>")?;
                        } else {
                            self.output.write_all(b"<ul")?;
                            self.render_sourcepos(node)?;
                            self.output.write_all(b">\n")?;
                        }
                    } else if nl.start == 1 {
                        self.output.write_all(b"<ol")?;
                        self.render_sourcepos(node)?;
                        self.output.write_all(b">\n")?;
                    } else {
                        self.output.write_all(b"<ol")?;
                        self.render_sourcepos(node)?;
                        writeln!(self.output, " start=\"{}\">", nl.start)?;
                    }
                } else if nl.list_type == ListType::Bullet {
                    if has_task_children(node) {
                        self.output.write_all(b"</ac:task-list>\n")?;
                    } else {
                        self.output.write_all(b"</ul>\n")?;
                    }
                } else {
                    self.output.write_all(b"</ol>\n")?;
                }
            }
            NodeValue::Item(..) => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<li")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</li>\n")?;
                }
            }
            NodeValue::DescriptionList => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<dl")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</dl>\n")?;
                }
            }
            NodeValue::DescriptionItem(..) => (),
            NodeValue::DescriptionTerm => {
                if entering {
                    self.output.write_all(b"<dt")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</dt>\n")?;
                }
            }
            NodeValue::DescriptionDetails => {
                if entering {
                    self.output.write_all(b"<dd")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</dd>\n")?;
                }
            }
            NodeValue::Heading(ref nch) => {
                if entering {
                    self.cr()?;
                    write!(self.output, "<h{}", nch.level)?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    writeln!(self.output, "</h{}>", nch.level)?;
                }
            }
            NodeValue::CodeBlock(ref ncb) => {
                if entering {
                    self.cr()?;

                    self.output.write_all(br#"<ac:structured-macro ac:name="code" ac:schema-version="1" ac:macro-id="d248891e-ba87-4ba9-becf-edfb21175463">"#)?;

                    self.output
                        .write_all(br#"<ac:parameter ac:name="language">"#)?;
                    self.output.write_all(ncb.info.as_bytes())?;
                    self.output.write_all(b"</ac:parameter>")?;
                    self.output.write_all(b"<ac:plain-text-body><![CDATA[")?;

                    let literal = &ncb.literal.trim_end().as_bytes();
                    self.output.write_all(literal)?;
                    self.output
                        .write_all(b"]]></ac:plain-text-body></ac:structured-macro>")?;
                }
            }
            NodeValue::HtmlBlock(ref nhb) => {
                // No sourcepos.
                if entering {
                    self.cr()?;
                    let literal = nhb.literal.as_bytes();
                    if self.options.render.escape {
                        self.escape(literal)?;
                    } else if !self.options.render.unsafe_ {
                        self.output.write_all(b"<!-- raw HTML omitted -->")?;
                    } else if self.options.extension.tagfilter {
                        tagfilter_block(literal, &mut self.output)?;
                    } else {
                        self.output.write_all(literal)?;
                    }
                    self.cr()?;
                }
            }
            NodeValue::ThematicBreak => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<hr")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b" />\n")?;
                }
            }
            NodeValue::Paragraph => {
                let tight = match node
                    .parent()
                    .and_then(|n| n.parent())
                    .map(|n| n.data.borrow().value.clone())
                {
                    Some(NodeValue::List(nl)) => nl.tight,
                    _ => false,
                };

                let tight = tight
                    || matches!(
                        node.parent().map(|n| n.data.borrow().value.clone()),
                        Some(NodeValue::DescriptionTerm)
                    );

                if !tight {
                    if entering {
                        self.cr()?;
                        self.output.write_all(b"<p")?;
                        self.render_sourcepos(node)?;
                        self.output.write_all(b">")?;
                    } else {
                        if let NodeValue::FootnoteDefinition(nfd) =
                            &node.parent().unwrap().data.borrow().value
                        {
                            if node.next_sibling().is_none() {
                                self.output.write_all(b" ")?;
                                self.put_footnote_backref(nfd)?;
                            }
                        }
                        self.output.write_all(b"</p>\n")?;
                    }
                }
            }
            NodeValue::Text(ref literal) => {
                if entering {
                    // self.escape(literal.as_bytes())?;
                    self.output.write_all(literal.as_bytes())?; // need to avoid escaping template stuff :/
                }
            }
            NodeValue::LineBreak => {
                if entering {
                    self.output.write_all(b"<br")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b" />\n")?;
                }
            }
            NodeValue::SoftBreak => {
                if entering {
                    if self.options.render.hardbreaks {
                        self.output.write_all(b"<br")?;
                        self.render_sourcepos(node)?;
                        self.output.write_all(b" />\n")?;
                    } else {
                        // confluence will keep the \n as a hard break anyways... replace with space
                        self.output.write_all(b" ")?;
                    }
                }
            }
            NodeValue::Code(NodeCode { ref literal, .. }) => {
                if entering {
                    self.output.write_all(b"<code")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                    self.escape(literal.as_bytes())?;
                    self.output.write_all(b"</code>")?;
                }
            }
            NodeValue::HtmlInline(ref literal) => {
                // No sourcepos.
                if entering {
                    let literal = literal.as_bytes();
                    if self.options.render.escape {
                        self.escape(literal)?;
                    } else if !self.options.render.unsafe_ {
                        self.output.write_all(b"<!-- raw HTML omitted -->")?;
                    } else if self.options.extension.tagfilter && tagfilter(literal) {
                        self.output.write_all(b"&lt;")?;
                        self.output.write_all(&literal[1..])?;
                    } else {
                        self.output.write_all(literal)?;
                    }
                }
            }
            NodeValue::Strong => {
                let parent_node = node.parent();
                if parent_node.is_none()
                    || !matches!(parent_node.unwrap().data.borrow().value, NodeValue::Strong)
                {
                    if entering {
                        self.output.write_all(b"<strong")?;
                        self.render_sourcepos(node)?;
                        self.output.write_all(b">")?;
                    } else {
                        self.output.write_all(b"</strong>")?;
                    }
                }
            }
            NodeValue::Emph => {
                if entering {
                    self.output.write_all(b"<em")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</em>")?;
                }
            }
            NodeValue::Strikethrough => {
                if entering {
                    self.output.write_all(b"<del")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</del>")?;
                }
            }
            NodeValue::Superscript => {
                if entering {
                    self.output.write_all(b"<sup")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.output.write_all(b"</sup>")?;
                }
            }
            NodeValue::Link(ref nl) => {
                let link_generator = self.link_generator;
                if entering {
                    let no_children = node.first_child().is_none();
                    link_generator.enter(nl, self, no_children)?;
                } else {
                    link_generator.exit(nl, self)?;
                }
            }
            NodeValue::Image(ref nl) => {
                if entering {
                    render_link_enter(nl, self.output)?;
                    return Ok(true);
                } else {
                    render_link_leave(nl, self.output)?;
                }
            }
            NodeValue::ShortCode(ref nsc) => {
                if entering {
                    self.output.write_all(nsc.emoji.as_bytes())?;
                }
            }
            NodeValue::Table(..) => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<table")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">\n")?;
                } else {
                    if !node
                        .last_child()
                        .unwrap()
                        .same_node(node.first_child().unwrap())
                    {
                        self.cr()?;
                        self.output.write_all(b"</tbody>\n")?;
                    }
                    self.cr()?;
                    self.output.write_all(b"</table>\n")?;
                }
            }
            NodeValue::TableRow(header) => {
                if entering {
                    self.cr()?;
                    if header {
                        self.output.write_all(b"<thead>\n")?;
                    } else if let Some(n) = node.previous_sibling() {
                        if let NodeValue::TableRow(true) = n.data.borrow().value {
                            self.output.write_all(b"<tbody>\n")?;
                        }
                    }
                    self.output.write_all(b"<tr")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b">")?;
                } else {
                    self.cr()?;
                    self.output.write_all(b"</tr>")?;
                    if header {
                        self.cr()?;
                        self.output.write_all(b"</thead>")?;
                    }
                }
            }
            NodeValue::TableCell => {
                let row = &node.parent().unwrap().data.borrow().value;
                let in_header = match *row {
                    NodeValue::TableRow(header) => header,
                    _ => panic!(),
                };

                let table = &node.parent().unwrap().parent().unwrap().data.borrow().value;
                let alignments = match *table {
                    NodeValue::Table(NodeTable { ref alignments, .. }) => alignments,
                    _ => panic!(),
                };

                if entering {
                    self.cr()?;
                    if in_header {
                        self.output.write_all(b"<th")?;
                        self.render_sourcepos(node)?;
                    } else {
                        self.output.write_all(b"<td")?;
                        self.render_sourcepos(node)?;
                    }

                    let mut start = node.parent().unwrap().first_child().unwrap();
                    let mut i = 0;
                    while !start.same_node(node) {
                        i += 1;
                        start = start.next_sibling().unwrap();
                    }

                    match alignments[i] {
                        TableAlignment::Left => {
                            self.output.write_all(b" align=\"left\"")?;
                        }
                        TableAlignment::Right => {
                            self.output.write_all(b" align=\"right\"")?;
                        }
                        TableAlignment::Center => {
                            self.output.write_all(b" align=\"center\"")?;
                        }
                        TableAlignment::None => (),
                    }

                    self.output.write_all(b">")?;
                } else if in_header {
                    self.output.write_all(b"</th>")?;
                } else {
                    self.output.write_all(b"</td>")?;
                }
            }
            NodeValue::FootnoteDefinition(ref nfd) => {
                if entering {
                    if self.footnote_ix == 0 {
                        self.output.write_all(b"<section")?;
                        self.render_sourcepos(node)?;
                        self.output
                            .write_all(b" class=\"footnotes\" data-footnotes>\n<ol>\n")?;
                    }
                    self.footnote_ix += 1;
                    self.output.write_all(b"<li")?;
                    self.render_sourcepos(node)?;
                    self.output.write_all(b" id=\"fn-")?;
                    self.escape_href(nfd.name.as_bytes())?;
                    self.output.write_all(b"\">")?;
                } else {
                    if self.put_footnote_backref(nfd)? {
                        self.output.write_all(b"\n")?;
                    }
                    self.output.write_all(b"</li>\n")?;
                }
            }
            NodeValue::FootnoteReference(ref nfr) => {
                if entering {
                    let mut ref_id = format!("fnref-{}", nfr.name);

                    self.output.write_all(b"<sup")?;
                    self.render_sourcepos(node)?;

                    if nfr.ref_num > 1 {
                        ref_id = format!("{}-{}", ref_id, nfr.ref_num);
                    }

                    self.output
                        .write_all(b" class=\"footnote-ref\"><a href=\"#fn-")?;
                    self.escape_href(nfr.name.as_bytes())?;
                    self.output.write_all(b"\" id=\"")?;
                    self.escape_href(ref_id.as_bytes())?;
                    write!(self.output, "\" data-footnote-ref>{}</a></sup>", nfr.ix)?;
                }
            }
            NodeValue::TaskItem(symbol) => {
                if entering {
                    self.cr()?;
                    self.output.write_all(b"<ac:task><ac:task-id>")?;
                    self.output
                        .write_all(self.next_task_id.to_string().as_bytes())?;
                    self.next_task_id += 1;
                    self.output.write_all(b"</ac:task-id><ac:task-status>")?;
                    if symbol.is_some() {
                        self.output.write_all(b"complete")?;
                    } else {
                        self.output.write_all(b"incomplete")?;
                    }
                    self.output.write_all(b"</ac:task-status><ac:task-body>")?;
                } else {
                    self.output.write_all(b"</ac:task-body></ac:task>\n")?;
                }
            }
            NodeValue::Raw(_) => (),
            NodeValue::Math(ref _node_math) => (),
            NodeValue::MultilineBlockQuote(_node_multiline_block_quote) => (),
            NodeValue::Escaped => (),
            NodeValue::WikiLink(ref _node_wiki_link) => (),
            NodeValue::Underline => (),
            NodeValue::Subscript => (),
            NodeValue::SpoileredText => (),
            NodeValue::EscapedTag(_) => (),
            NodeValue::Alert(ref node_alert) => self.render_alert(entering, node_alert)?,
        }
        Ok(false)
    }

    fn render_alert(
        &mut self,
        entering: bool,
        node_alert: &comrak::nodes::NodeAlert,
    ) -> Result<(), io::Error> {
        if let Some(title) = &node_alert.title {
            if title.starts_with("[expand]") {
                return render_expand(self.output, title, entering);
            }
        }
        render_basic_alert(self.output, node_alert, entering)
    }

    fn render_sourcepos<'a>(&mut self, node: &'a AstNode<'a>) -> io::Result<()> {
        if self.options.render.sourcepos {
            let ast = node.data.borrow();
            if ast.sourcepos.start.line > 0 {
                write!(self.output, " data-sourcepos=\"{}\"", ast.sourcepos)?;
            }
        }
        Ok(())
    }

    fn put_footnote_backref(&mut self, nfd: &NodeFootnoteDefinition) -> io::Result<bool> {
        if self.written_footnote_ix >= self.footnote_ix {
            return Ok(false);
        }

        self.written_footnote_ix = self.footnote_ix;

        let mut ref_suffix = String::new();
        let mut superscript = String::new();

        for ref_num in 1..=nfd.total_references {
            if ref_num > 1 {
                ref_suffix = format!("-{}", ref_num);
                superscript = format!("<sup class=\"footnote-ref\">{}</sup>", ref_num);
                write!(self.output, " ")?;
            }

            self.output.write_all(b"<a href=\"#fnref-")?;
            self.escape_href(nfd.name.as_bytes())?;
            write!(
                self.output,
                "{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}{}\" aria-label=\"Back to reference {}{}\">↩{}</a>",
                ref_suffix, self.footnote_ix, ref_suffix, self.footnote_ix, ref_suffix, superscript
            )?;
        }
        Ok(true)
    }
}

fn has_task_children<'a>(
    node: &'a comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>>,
) -> bool {
    let result = node
        .children()
        .any(|child_node| matches!(child_node.data.borrow().value, NodeValue::TaskItem(_)));

    result
}
