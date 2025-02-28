use std::io::Read;

use saphyr::Yaml;

use crate::Result;
use thiserror::Error;

// fn read_frontmatter_as_yaml(filename: &str) -> Yaml {
//     Yaml::from_str("foo")
// }

#[derive(Error, Debug)]
pub enum FrontMatterError {
    #[error("Unable to parse front matter: {0}")]
    ParseError(String),
}

#[derive(Debug)]
pub struct FrontMatter {
    pub labels: Vec<String>,
    pub emoji: String,
    pub metadata: Yaml,
    pub unknown_keys: Vec<String>,
}

enum ParseState {
    BeforeFrontMatter,
    InFrontMatter,
}

impl FrontMatter {
    pub fn default() -> FrontMatter {
        FrontMatter {
            labels: Vec::default(),
            emoji: String::default(),
            metadata: Yaml::Null,
            unknown_keys: Vec::default(),
        }
    }

    pub fn from_reader(reader: &impl Read) -> Result<FrontMatter> {
        Ok(FrontMatter::default())
    }

    #[cfg(test)]
    pub fn from_str(s: &str) -> Result<FrontMatter> {
        use std::{
            collections::HashSet,
            io::{self, BufRead, Cursor},
        };

        use anyhow::Context;

        let mut front_matter_str = String::new();
        let mut state = ParseState::BeforeFrontMatter;
        let cursor = Cursor::new(s);
        let lines = io::BufReader::new(cursor).lines();
        for line in lines.map_while(io::Result::ok) {
            match state {
                ParseState::BeforeFrontMatter => {
                    let trimmed_line = line.trim();
                    if trimmed_line == "---" {
                        state = ParseState::InFrontMatter;
                    } else if !trimmed_line.is_empty() {
                        // found non frontmatter marker, assuming no front matter
                        break;
                    } else {
                        // whitespace before front matter
                    }
                }
                ParseState::InFrontMatter => {
                    if line.starts_with("---") {
                        break; // end of front matter
                    } else {
                        front_matter_str.push_str(&line);
                        front_matter_str += "\n";
                    }
                }
            }
        }

        if front_matter_str.is_empty() {
            return Ok(FrontMatter::default());
        }

        let yaml_fm_docs = Yaml::load_from_str(&front_matter_str)
            .context("Failed to parse front matter as YAML")?;
        let yaml_fm = &yaml_fm_docs[0];
        if !yaml_fm.is_hash() {
            return Err(FrontMatterError::ParseError(String::from(
                "Expected YAML hash map for front matter",
            ))
            .into());
        }

        static VALID_TOP_LEVEL_KEYS: [&str; 3] = ["emoji", "labels", "metadata"];
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
        //
        //                 if !unknown_keys.is_empty() {
        //                     warnings.push(format!(
        //                         "Unknown top level front matter keys: {}",
        //                         unknown_keys.join(", "),
        //                     ));
        //                 }
        //                 front_matter = yaml.clone();

        println!("Yaml fm {:?}", yaml_fm);
        let mut labels: Vec<String> = Vec::default();

        if let Some(parsed_labels) = yaml_fm["labels"].as_vec().map(|v| {
            v.iter()
                .map(|l| String::from(l.as_str().unwrap()))
                .collect::<Vec<String>>()
        }) {
            labels = parsed_labels;
        }

        let emoji = String::from(yaml_fm["emoji"].as_str().unwrap_or_default());
        Ok(FrontMatter {
            labels,
            emoji,
            metadata: yaml_fm["metadata"].clone(),
            unknown_keys,
        })
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
        let fm = FrontMatter::from_str(FRONT_MATTER_MD)?;

        assert_eq!(fm.labels, vec!["foo", "bar"]);
        assert_eq!(fm.emoji, "heart_eyes");
        assert_eq!(fm.metadata["some"]["arbitrary"].as_str(), Some("value"));

        Ok(())
    }

    #[test]
    fn it_returns_empty_frontmatter_if_not_present() -> TestResult {
        let fm = FrontMatter::from_str(EMPTY_MD)?;
        assert_eq!(fm.labels, Vec::<String>::default());
        assert_eq!(fm.emoji, String::default());
        assert!(fm.metadata.is_null());
        Ok(())
    }

    #[test]
    fn it_ignores_spaces_before_frontmatter() -> TestResult {
        let fm = FrontMatter::from_str(&(String::from("\n   \n") + FRONT_MATTER_MD))?;
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
}
