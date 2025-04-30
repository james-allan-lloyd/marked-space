use saphyr::Yaml;

use crate::{sort::Sort, Result};
use std::{
    collections::HashSet,
    io::{self, BufRead},
};
use thiserror::Error;

use anyhow::Context;

#[derive(Error, Debug)]
pub enum FrontMatterError {
    #[error("Unable to parse front matter: {0}")]
    ParseError(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct FrontMatter {
    pub labels: Vec<String>,
    pub emoji: String,
    pub metadata: Yaml,
    pub unknown_keys: Vec<String>,
    pub imports: Vec<String>,
    pub folder: bool,
    pub sort: Sort,
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
            metadata: Yaml::Null,
            unknown_keys: Vec::default(),
            imports: Vec::default(),
            folder: false,
            sort: Sort::Unsorted,
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

        let yaml_fm_docs = Yaml::load_from_str(&front_matter_str)
            .context("Failed to parse front matter as YAML")?;
        if yaml_fm_docs.is_empty() {
            return Ok((FrontMatter::default(), content_str));
        }
        let yaml_fm = &yaml_fm_docs[0];
        if !yaml_fm.is_hash() {
            return Err(FrontMatterError::ParseError(String::from(
                "Expected YAML hash map for front matter",
            ))
            .into());
        }

        static VALID_TOP_LEVEL_KEYS: [&str; 5] =
            ["emoji", "labels", "metadata", "imports", "folder"];
        let string_keys: HashSet<&str> = yaml_fm
            .as_hash()
            .unwrap()
            .iter()
            .map(|(key, _value)| key.as_str().unwrap())
            .collect();

        let mut unknown_keys: Vec<String> = string_keys
            .difference(&HashSet::from(VALID_TOP_LEVEL_KEYS))
            .map(|s| s.to_string())
            .collect();

        unknown_keys.sort();

        let labels = yaml_fm["labels"]
            .as_vec()
            .map(|v| {
                v.iter()
                    .map(|l| String::from(l.as_str().unwrap()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let imports = yaml_fm["imports"]
            .as_vec()
            .map(|v| {
                v.iter()
                    .map(|l| String::from(l.as_str().unwrap()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let folder = yaml_fm["folder"]
            .borrowed_or(&Yaml::Boolean(false))
            .as_bool()
            .ok_or(anyhow::anyhow!(
                "Failed to parse \"folder\" key (should be true/false)"
            ))?;

        let emoji = String::from(yaml_fm["emoji"].as_str().unwrap_or_default());

        let sort = Sort::from_str(yaml_fm["sort"].as_str())?;

        Ok((
            FrontMatter {
                labels,
                emoji,
                metadata: yaml_fm["metadata"].clone(),
                unknown_keys,
                imports,
                folder,
                sort,
            },
            content_str,
        ))
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
    static NOT_YAML_FRONT_MATTER_MD: &str = r##"---
something = foo
[section]
gah = bar
---
# compulsory title
"##;

    use crate::error::TestResult;

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
    fn it_errors_if_not_yaml() -> TestResult {
        let fm_result = FrontMatter::from_str(NOT_YAML_FRONT_MATTER_MD);

        println!("{:?}", fm_result);

        assert!(fm_result.is_err());
        if let Err(err) = fm_result {
            assert_eq!(
                err.to_string(),
                "Unable to parse front matter: Expected YAML hash map for front matter"
            );
        }

        Ok(())
    }

    #[test]
    fn it_parses_yes_as_true() -> TestResult {
        let fm_result = FrontMatter::from_str("---\nfolder: yes\n---\n# title");

        assert!(fm_result.is_err());
        assert_eq!(
            fm_result.err().unwrap().to_string(),
            "Failed to parse \"folder\" key (should be true/false)"
        );

        Ok(())
    }

    #[test]
    fn it_parses_front_matter_that_is_only_a_comment() {
        let fm_result = FrontMatter::from_str("---\n#comment\n---\n# title");

        assert!(fm_result.is_ok());

        let (fm, _content) = fm_result.unwrap();
        assert_eq!(fm, FrontMatter::default());
    }
}
