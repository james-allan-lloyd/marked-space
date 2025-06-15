use crate::page_covers::string_or_struct;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::{page_covers::Cover, page_statuses::PageStatus, sort::Sort, Result};
use std::io::{self, BufRead};

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct FrontMatter {
    pub labels: Vec<String>,
    pub emoji: String,
    #[serde(deserialize_with = "string_or_struct")]
    pub cover: Cover,
    pub metadata: tera::Value,
    pub unknown_keys: Vec<String>,
    pub imports: Vec<String>,
    pub folder: bool,
    pub sort: Sort,
    pub status: Option<PageStatus>,
}

enum FrontMatterParseState {
    Before,
    Inside,
    After,
}

impl Default for FrontMatter {
    fn default() -> Self {
        FrontMatter {
            labels: Vec::default(),
            emoji: String::default(),
            metadata: tera::Value::Null,
            unknown_keys: Vec::default(),
            imports: Vec::default(),
            folder: false,
            sort: Sort::Unsorted,
            cover: Cover::default(),
            status: None,
        }
    }
}

impl FrontMatter {
    #[cfg(test)]
    pub fn from_str(s: &str) -> Result<(FrontMatter, String)> {
        use std::io::Cursor;
        Self::from_reader(&mut Cursor::new(s))
    }

    pub fn from_reader(reader: &mut dyn std::io::BufRead) -> Result<(FrontMatter, String)> {
        let mut front_matter_str = String::new();
        let mut content_str = String::new();
        let mut state = FrontMatterParseState::Before;
        let lines = reader.lines();
        for line in lines.map_while(io::Result::ok) {
            match state {
                FrontMatterParseState::Before => {
                    let trimmed_line = line.trim();
                    if trimmed_line == "---" {
                        state = FrontMatterParseState::Inside;
                    } else if !trimmed_line.is_empty() {
                        // found non frontmatter marker, assuming no front matter
                        state = FrontMatterParseState::After;
                        content_str.push_str(&line);
                        content_str += "\n";
                    } else {
                        // whitespace before front matter
                    }
                }
                FrontMatterParseState::Inside => {
                    if line.starts_with("---") {
                        state = FrontMatterParseState::After;
                    } else {
                        front_matter_str.push_str(&line);
                        front_matter_str += "\n";
                    }
                }
                FrontMatterParseState::After => {
                    content_str.push_str(&line);
                    content_str += "\n";
                }
            }
        }

        let front_matter: FrontMatter =
            match saphyr_serde::de::from_str::<Option<FrontMatter>>(&front_matter_str) {
                Ok(optional_fm) => optional_fm.unwrap_or_default(),
                Err(err) => Err(anyhow!("Failed to parse: {:?}", err))?,
            };

        Ok((front_matter, content_str))
    }
}

#[cfg(test)]
mod tests {

    static FRONT_MATTER_MD: &str = r##"---
labels:
- foo
- bar
emoji: heart_eyes
metadata:
    some:
        arbitrary: "value"
---
# compulsory title
{{ metadata.some.arbitrary }}
"##;

    static EMPTY_MD: &str = "# compulsory title\n";

    use crate::{error::TestResult, test_helpers::test_render};

    use super::*;

    #[test]
    fn it_reads_frontmatter() -> TestResult {
        let (fm, _content) = FrontMatter::from_str(FRONT_MATTER_MD)?;

        assert_eq!(fm.labels, vec!["foo", "bar"]);
        assert_eq!(fm.emoji, "heart_eyes");
        assert_eq!(fm.metadata["some"]["arbitrary"].as_str(), Some("value"));

        assert_eq!(
            _content,
            "# compulsory title\n{{ metadata.some.arbitrary }}\n"
        );

        Ok(())
    }

    #[test]
    fn it_returns_empty_frontmatter_if_not_present() -> TestResult {
        let (fm, _content) = FrontMatter::from_str(EMPTY_MD)?;
        assert_eq!(fm.labels, Vec::<String>::default());
        assert_eq!(fm.emoji, String::default());
        assert!(fm.metadata.is_null());
        Ok(())
    }

    #[test]
    fn it_ignores_spaces_before_frontmatter() -> TestResult {
        let (fm, _content) = FrontMatter::from_str(&(String::from("\n   \n") + FRONT_MATTER_MD))?;
        assert_eq!(fm.labels, vec!["foo", "bar"]);
        assert_eq!(fm.emoji, "heart_eyes");
        assert_eq!(fm.metadata["some"]["arbitrary"].as_str(), Some("value"));
        Ok(())
    }

    #[test]
    fn it_parses_yes_as_true() -> TestResult {
        let (fm, _content) =
            FrontMatter::from_str("---\nfolder: yes\n---\n# title").expect("Should parse");

        assert!(fm.folder);

        Ok(())
    }

    #[test]
    fn it_parses_front_matter_that_is_only_a_comment() {
        let (fm, _content) =
            FrontMatter::from_str("---\n#comment\n---\n# title").expect("Should pass");

        assert_eq!(fm, FrontMatter::default());
    }

    #[test]
    fn it_renders_front_matter_variable() -> TestResult {
        let rendered_page = test_render(
            r###"---
metadata:
    owner: James
    status: complete
---
# Title

{{ fm.metadata.owner }}: {{ fm.metadata.status }}
"###,
        )?;

        assert_eq!(rendered_page.content.trim(), "<p>James: complete</p>");

        Ok(())
    }
}
