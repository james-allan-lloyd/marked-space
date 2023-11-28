use clap::builder::OsStr;
use walkdir::WalkDir;

use crate::error::{ConfluenceError, Result};
use std::path::{Path, PathBuf};

pub struct MarkdownSpace {
    markdown_pages: Vec<PathBuf>,
}

impl From<walkdir::Error> for ConfluenceError {
    fn from(value: walkdir::Error) -> Self {
        ConfluenceError::new(format!(
            "Failed to iterate space files: {}",
            value.to_string()
        ))
    }
}

impl MarkdownSpace {
    fn from_directory(dir: &Path) -> Result<MarkdownSpace> {
        println!("space dir {}", dir.display());
        let mut markdown_pages = Vec::<PathBuf>::default();
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().extension() == Some(&OsStr::from("md")) {
                markdown_pages.push(entry.into_path());
            }
        }
        if dir.exists() {
            Ok(MarkdownSpace { markdown_pages })
        } else {
            Err(crate::error::ConfluenceError::new(
                "Space directory does not exist",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assert_fs::fixture::{FileTouch, PathChild};

    use crate::error::ConfluenceError;

    use super::MarkdownSpace;

    type Result = std::result::Result<(), ConfluenceError>;

    #[test]
    fn it_fails_if_space_directory_doesnt_exist() {
        let space = MarkdownSpace::from_directory(Path::new("test"));

        assert!(space.is_err(), "Should fail if directory does not exist");
    }

    #[test]
    fn it_find_markdown_files_in_space_directory() -> Result {
        let temp = assert_fs::TempDir::new().unwrap().into_persistent();
        let _markdown1 = temp.child("test/markdown1.md").touch().unwrap();
        let _markdown2 = temp.child("test/markdown2.md").touch().unwrap();
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;

        assert_eq!(space.markdown_pages.len(), 2);

        Ok(())
    }

    fn _it_fails_if_space_directory_is_invalid_space_key() {}
}
