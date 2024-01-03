use std::path::PathBuf;

use crate::Result;
use crate::{error::ConfluenceError, html::LinkGenerator};

pub fn get_parent_title(
    page_path: PathBuf,
    link_generator: &LinkGenerator,
) -> Result<Option<String>> {
    if let Some(parent_path) = page_path.parent() {
        if parent_path == PathBuf::default() {
            // Parent is space
            Ok(None)
        } else {
            let mut parent_path = PathBuf::from(parent_path);
            if page_path.file_name().unwrap() == "index.md" {
                parent_path.pop();
            }
            if parent_path == PathBuf::default() {
                return Ok(None);
            }
            let parent_path_page = parent_path.join("index.md");
            println!("parent: {:#?}", parent_path_page.as_path().display());
            if let Some(parent_title) = link_generator.get_file_title(&parent_path_page).cloned() {
                Ok(Some(parent_title))
            } else {
                Err(ConfluenceError::generic_error(format!(
                    "Missing parent: {}",
                    parent_path_page.display()
                )))
            }
        }
    } else {
        // Parent is space
        Ok(None)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    type TestResult = std::result::Result<(), anyhow::Error>;

    #[test]
    fn it_returns_none_for_top_level_pages() -> TestResult {
        // let temp = assert_fs::TempDir::new().unwrap();
        // let top_level_page = temp.child("test/markdown1.md");
        // top_level_page.touch()?;

        let parent_title = get_parent_title(PathBuf::from("markdown1.md"), &LinkGenerator::new())?;

        assert_eq!(parent_title, None);

        Ok(())
    }

    #[test]
    fn it_returns_title_for_subpages() -> TestResult {
        let mut link_generator = LinkGenerator::new();
        link_generator.add_file_title(
            &PathBuf::from("subpages").join("index.md"),
            &String::from("Page with Sub Pages"),
        )?;
        let parent_title =
            get_parent_title(PathBuf::from("subpages/markdown1.md"), &link_generator)?;
        assert!(parent_title.is_some());
        assert_eq!(parent_title.unwrap(), String::from("Page with Sub Pages"));
        Ok(())
    }

    #[test]
    fn it_returns_an_error_if_parent_page_doesnt_exist() -> TestResult {
        let parent_title = get_parent_title(
            PathBuf::from("subpages/markdown1.md"),
            &LinkGenerator::new(),
        );

        assert!(parent_title.is_err());

        Ok(())
    }

    #[test]
    fn it_returns_parent_for_index_md() -> TestResult {
        let parent_title =
            get_parent_title(PathBuf::from("subpages/index.md"), &LinkGenerator::new())?;

        assert!(parent_title.is_none());

        Ok(())
    }
}
