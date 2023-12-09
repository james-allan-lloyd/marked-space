use clap::builder::OsStr;
use walkdir::WalkDir;

use crate::error::Result;
use std::path::{Path, PathBuf};

pub struct MarkdownSpace {
    pub key: String,
    pub markdown_pages: Vec<PathBuf>,
    pub dir: PathBuf,
}

impl MarkdownSpace {
    pub fn from_directory(dir: &Path) -> Result<MarkdownSpace> {
        let mut markdown_pages = Vec::<PathBuf>::default();
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().extension() == Some(&OsStr::from("md")) {
                markdown_pages.push(entry.into_path());
            }
        }
        let key = String::from(dir.file_stem().unwrap().to_str().unwrap()).to_uppercase();
        if dir.exists() {
            Ok(MarkdownSpace {
                markdown_pages,
                key,
                dir: PathBuf::from(dir),
            })
        } else {
            Err(crate::error::ConfluenceError::generic_error(
                "Space directory does not exist",
            ))
        }
    }

    pub fn relative_page_path(&self, page_path: &Path) -> PathBuf {
        page_path.strip_prefix(&self.dir).unwrap().into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assert_fs::fixture::{FileTouch, PathChild};

    use super::MarkdownSpace;

    type Result = std::result::Result<(), anyhow::Error>;

    #[test]
    fn it_fails_if_space_directory_doesnt_exist() {
        let space = MarkdownSpace::from_directory(Path::new("test"));

        assert!(space.is_err(), "Should fail if directory does not exist");
    }

    #[test]
    fn it_find_markdown_files_in_space_directory() -> Result {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/markdown1.md").touch().unwrap();
        temp.child("test/markdown2.md").touch().unwrap();
        let space = MarkdownSpace::from_directory(temp.child("test").path())?;

        assert_eq!(space.markdown_pages.len(), 2);

        Ok(())
    }

    #[test]
    fn it_uses_the_basename_of_current_directory_if_not_full_path() -> Result {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/markdown1.md").touch().unwrap();
        temp.child("test/markdown2.md").touch().unwrap();

        let old_pwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path())?;

        let result = MarkdownSpace::from_directory(temp.child("test").path());

        std::env::set_current_dir(old_pwd)?;

        assert!(result.is_ok());
        assert_eq!(result?.key, "TEST");

        Ok(())
    }

    fn _it_fails_if_space_directory_is_invalid_spacek_key() {}
}
