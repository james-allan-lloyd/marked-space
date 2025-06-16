use crate::error::{ConfluenceError, Result};
use std::{
    fmt::{Display, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, PartialEq)]
pub struct LocalLink {
    pub path: PathBuf,
    pub anchor: Option<String>,
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
    pub fn from_str(s: &str, relative_path: &Path) -> Result<Self> {
        let result = if let Some(hash_pos) = s.find('#') {
            let (p, a) = s.split_at(hash_pos);
            if a.len() <= 1 {
                return Err(ConfluenceError::generic_error("Cannot have empty anchors"));
            }
            LocalLink {
                path: simplify_path(&relative_path.join(PathBuf::from_str(p)?))?,
                anchor: Some(String::from(&a[1..])),
            }
        } else {
            LocalLink {
                path: simplify_path(&relative_path.join(PathBuf::from_str(s)?))?,
                anchor: None,
            }
        };
        Ok(result)
    }
}

impl Display for LocalLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.path.to_str().unwrap().replace('\\', "/").as_str())?;
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

    use crate::error::TestResult;

    use super::LocalLink;

    #[test]
    fn it_parses_local_links_without_anchor() -> TestResult {
        let local_link = LocalLink::from_str("test.md", &PathBuf::default())?;
        assert_eq!(local_link.path, PathBuf::from_str("test.md")?);
        assert_eq!(local_link.anchor, None);

        Ok(())
    }

    #[test]
    fn it_parses_local_links_with_anchor() -> TestResult {
        let local_link = LocalLink::from_str("test.md#anchor", &PathBuf::default())?;
        assert_eq!(local_link.path, PathBuf::from_str("test.md")?);
        assert_eq!(local_link.anchor, Some(String::from("anchor")));

        Ok(())
    }

    #[test]
    fn it_errors_with_empty_anchor() -> TestResult {
        let result = LocalLink::from_str("test.md#a", &PathBuf::default());
        assert!(result.is_ok());
        let result = LocalLink::from_str("test.md#", &PathBuf::default());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn it_simplifies_relative_links() -> TestResult {
        let local_link = LocalLink::from_str("../test.md#a", &PathBuf::from("subdir"))?;
        assert_eq!(local_link.path, PathBuf::from_str("test.md")?);

        Ok(())
    }

    #[test]
    fn it_errors_if_link_is_outside_of_space() -> TestResult {
        let result = LocalLink::from_str("../test.md#a", &PathBuf::default());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn it_parses_local_links() -> TestResult {
        let result = LocalLink::from_str("#anchor", &PathBuf::default());
        println!("{:#?}", result);
        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn it_converts_to_string_with_platform_independent_separator() -> TestResult {
        let local_link = LocalLink::from_str("foo/bar#baz", &PathBuf::default())?;
        assert_eq!(local_link.to_string(), "foo/bar#baz");
        Ok(())
    }
}
