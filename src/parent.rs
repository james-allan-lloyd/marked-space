use std::path::{Path, PathBuf};

pub fn get_parent_file(page_path: &Path) -> Option<PathBuf> {
    if let Some(parent_path) = page_path.parent() {
        if parent_path == PathBuf::default() {
            // Parent is space
            None
        } else {
            let mut parent_path = PathBuf::from(parent_path);
            if page_path.file_name().unwrap() == "index.md" {
                parent_path.pop();
            }
            if parent_path == PathBuf::default() {
                return None;
            }
            return Some(parent_path.join("index.md"));
        }
    } else {
        return None;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_returns_none_for_top_level_pages() {
        let parent_file = get_parent_file(&PathBuf::from("markdown1.md"));

        assert_eq!(parent_file, None);
    }

    #[test]
    fn it_returns_title_for_subpages() {
        let parent_file = get_parent_file(&PathBuf::from("subpages/markdown1.md"));
        assert!(parent_file.is_some());
        assert_eq!(parent_file.unwrap(), PathBuf::from("subpages/index.md"));
    }

    #[test]
    fn it_returns_none_top_level_parent_page_index_md() {
        let parent_file = get_parent_file(&PathBuf::from("subpages/index.md"));

        assert_eq!(parent_file, None);
    }

    #[test]
    fn it_returns_none_parent_for_homepage() {
        let parent_file = get_parent_file(&PathBuf::from("index.md"));

        assert_eq!(parent_file, None);
    }
}
