use crate::{
    attachments::link_to_name,
    error::{ConfluenceError, Result},
};
use std::{
    fmt::{Display, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, PartialEq)]
pub struct LocalLink {
    pub page_path: PathBuf,     // the file the link was defined in
    pub text: String,           // the original text of the link
    pub target: PathBuf,        // the full path to the linked file
    pub anchor: Option<String>, // an optional anchor within the file
}

pub fn simplify_path(p: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();

    for c in p.components() {
        match c.as_os_str().to_str().unwrap() {
            ".." => {
                if !result.pop() {
                    return Err(ConfluenceError::generic_error(format!(
                        "Invalid link (goes outside of space tree): {}",
                        p.display()
                    )));
                }
            }
            "." => (),
            _ => result.push(c),
        }
    }

    Ok(result)
}

impl LocalLink {
    // TODO: make from_str return an optional local link?
    pub fn is_local_link(text: &str) -> bool {
        !(text.starts_with("http://") || text.starts_with("https://") || text.starts_with("ac:"))
    }

    fn split_anchor(text: &str, page_path: &Path) -> Result<(PathBuf, Option<String>)> {
        let page_dir = page_path.parent().expect("All pages should have parent");
        if let Some(hash_pos) = text.find('#') {
            let (p, a) = text.split_at(hash_pos);
            if a.len() <= 1 {
                return Err(ConfluenceError::generic_error("Cannot have empty anchors"));
            }
            if p.is_empty() {
                Ok((page_path.to_owned(), Some(String::from(&a[1..]))))
            } else {
                Ok((
                    simplify_path(&page_dir.join(PathBuf::from_str(p)?))?,
                    Some(String::from(&a[1..])),
                ))
            }
        } else {
            Ok((
                simplify_path(&page_dir.join(PathBuf::from_str(text)?))?,
                None,
            ))
        }
    }

    pub fn from_str(text: &str, page_path: &Path) -> Result<Self> {
        assert_eq!(
            page_path.extension().map(|x| x.display().to_string()),
            Some(String::from("md"))
        );
        let (mut target, anchor) = Self::split_anchor(text, page_path)?;
        if target.is_dir() {
            target.push("index.md");
        }
        Ok(LocalLink {
            text: text.to_owned(),
            page_path: page_path.to_owned(),
            target,
            anchor,
        })
    }

    pub fn attachment_name(&self) -> String {
        link_to_name(&self.text)
    }

    pub fn is_page(&self) -> bool {
        if let Some(ext) = self.target.extension() {
            ext == "md"
        } else {
            false
        }
    }
}

impl Display for LocalLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.target.to_str().unwrap().replace('\\', "/").as_str())?;
        if let Some(anchor) = &self.anchor {
            f.write_char('#')?;
            f.write_str(anchor.as_str())?;
        };
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr};

    use assert_fs::prelude::{FileTouch, PathChild};

    use crate::error::TestResult;

    use super::LocalLink;

    #[test]
    fn it_parses_local_links_without_anchor() -> TestResult {
        let local_link = LocalLink::from_str("test.md", &PathBuf::from("index.md"))?;
        assert_eq!(local_link.target, PathBuf::from_str("test.md")?);
        assert_eq!(local_link.anchor, None);

        Ok(())
    }

    #[test]
    fn it_parses_local_links_with_anchor() -> TestResult {
        let local_link = LocalLink::from_str("test.md#anchor", &PathBuf::from("index.md"))?;
        assert_eq!(local_link.target, PathBuf::from_str("test.md")?);
        assert_eq!(local_link.anchor, Some(String::from("anchor")));

        Ok(())
    }

    #[test]
    fn it_errors_with_empty_anchor() -> TestResult {
        let result = LocalLink::from_str("test.md#a", &PathBuf::from("index.md"));
        assert!(result.is_ok());
        let result = LocalLink::from_str("test.md#", &PathBuf::from("index.md"));
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn it_simplifies_relative_links() -> TestResult {
        let local_link = LocalLink::from_str("../test.md#a", &PathBuf::from("subdir/index.md"))?;
        assert_eq!(local_link.target, PathBuf::from_str("test.md")?);

        Ok(())
    }

    #[test]
    fn it_errors_if_link_is_outside_of_space() -> TestResult {
        let result = LocalLink::from_str("../test.md#a", &PathBuf::from("index.md"));
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn it_parses_local_links() -> TestResult {
        let result = LocalLink::from_str("#anchor", &PathBuf::from("index.md"));
        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn it_converts_to_string_with_platform_independent_separator() -> TestResult {
        let local_link = LocalLink::from_str("foo/bar#baz", &PathBuf::from("index.md"))?;
        assert_eq!(local_link.to_string(), "foo/bar#baz");
        Ok(())
    }

    #[test]
    fn it_keeps_relative_part_for_name() -> TestResult {
        let local_link = LocalLink::from_str("./test.xls", &PathBuf::from("index.md"))?;
        assert_eq!(local_link.target, PathBuf::from_str("test.xls")?);
        assert_eq!(local_link.attachment_name(), "._test.xls");

        Ok(())
    }

    #[test]
    fn it_resolves_links_to_directories_to_index_md() -> TestResult {
        let temp = assert_fs::TempDir::new().unwrap();
        let index_md = temp.child("index.md");
        temp.child("testdir/index.md").touch().unwrap();
        let local_link = LocalLink::from_str("testdir", &index_md)?;
        let mut expected_taget = temp.to_path_buf();
        expected_taget.push(PathBuf::from("testdir/index.md"));
        assert_eq!(local_link.target, expected_taget);
        assert!(local_link.is_page());

        Ok(())
    }

    #[test]
    fn it_resolves_anchors_only_to_current_file() -> TestResult {
        let local_link = LocalLink::from_str("#code", &PathBuf::from("test.md"))?;
        let expected_target = PathBuf::from("test.md");
        assert_eq!(local_link.target, expected_target);
        assert!(local_link.is_page());
        assert_eq!(local_link.anchor, Some(String::from("code")));

        Ok(())
    }
}
