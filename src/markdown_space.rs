use clap::builder::OsStr;
use comrak::{nodes::AstNode, Arena};
use walkdir::WalkDir;

use crate::{
    error::{ConfluenceError, Result},
    html::LinkGenerator,
    markdown_page::MarkdownPage,
};
use std::path::{Path, PathBuf};

pub struct MarkdownSpace<'a> {
    pub key: String,
    pub arena: Arena<AstNode<'a>>,
    pub markdown_pages: Vec<PathBuf>,
    pub dir: PathBuf,
}

impl<'a> MarkdownSpace<'a> {
    pub fn from_directory(dir: &Path) -> Result<Self> {
        let mut markdown_pages = Vec::<PathBuf>::default();
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().is_dir() {
                if !entry.path().join("index.md").exists() {
                    println!(
                        "Warning: directory {} is missing index.md",
                        entry.path().display()
                    )
                }
            } else if entry.path().extension() == Some(&OsStr::from("md")) {
                markdown_pages.push(entry.into_path());
            }
        }
        let key = String::from(dir.file_stem().unwrap().to_str().unwrap()).to_uppercase();
        if dir.exists() {
            Ok(MarkdownSpace {
                markdown_pages,
                key,
                dir: PathBuf::from(dir),
                arena: Arena::new(),
            })
        } else {
            Err(crate::error::ConfluenceError::generic_error(
                "Space directory does not exist",
            ))
        }
    }

    pub fn relative_page_path(&self, page_path: &Path) -> Result<PathBuf> {
        match page_path.strip_prefix(&self.dir) {
            Ok(p) => Ok(PathBuf::from(p)),
            Err(_) => Err(ConfluenceError::generic_error(format!(
                "Page is not in space directory {}: {}",
                self.dir.display(),
                page_path.display()
            ))),
        }
    }

    pub(crate) fn parse(&'a self, link_generator: &mut LinkGenerator) -> Result<Vec<MarkdownPage>> {
        let mut parse_errors = Vec::<anyhow::Error>::default();
        let markdown_pages: Vec<MarkdownPage> = self
            .markdown_pages
            .iter()
            .map(|markdown_page_path| MarkdownPage::parse(self, markdown_page_path, &self.arena))
            .filter_map(|r| r.map_err(|e| parse_errors.push(e)).ok())
            .collect();

        let title_errors = markdown_pages
            .iter()
            .map(|markdown_page| {
                link_generator.add_file_title(
                    &PathBuf::from(markdown_page.source.clone()),
                    &markdown_page.title,
                )
            })
            .filter_map(|r| r.err());

        let missing_files = markdown_pages
            .iter()
            .flat_map(|markdown_page| {
                markdown_page.local_links.iter().map(|local_link| {
                    if self.dir.join(local_link).exists() {
                        Ok(local_link)
                    } else {
                        Err(ConfluenceError::MissingFileLink {
                            source_file: markdown_page.source.clone(),
                            local_link: local_link.display().to_string(),
                        }
                        .into())
                    }
                })
            })
            .filter_map(|r| r.err());

        let missing_attachments = markdown_pages
            .iter()
            .flat_map(|markdown_page| {
                markdown_page.attachments.iter().map(|attachment| {
                    if attachment.exists() {
                        Ok(attachment)
                    } else {
                        Err(ConfluenceError::MissingAttachmentLink {
                            source_file: markdown_page.source.clone(),
                            attachment_path: attachment
                                .strip_prefix(self.dir.as_path())
                                .unwrap()
                                .display()
                                .to_string(),
                        }
                        .into())
                    }
                })
            })
            .filter_map(|r| r.err());

        parse_errors.extend(title_errors);
        parse_errors.extend(missing_files);
        parse_errors.extend(missing_attachments);

        if !parse_errors.is_empty() {
            let error_string: String = parse_errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            return Err(ConfluenceError::generic_error(
                String::from("Error parsing space: ") + &error_string,
            ));
        }
        Ok(markdown_pages)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assert_fs::fixture::{FileTouch, FileWriteStr as _, PathChild};

    use crate::html::LinkGenerator;

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

    fn _it_fails_if_space_directory_is_invalid_space_key() {}
    fn _it_skips_subpages_if_directory_contains_no_markdown_files() {}

    #[test]
    fn it_fails_when_headings_are_duplicated() {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/markdown1.md")
            .write_str("# The Same Heading")
            .unwrap();
        temp.child("test/markdown2.md")
            .write_str("# The Same Heading")
            .unwrap();

        let result = MarkdownSpace::from_directory(temp.child("test").path());

        assert!(result.is_ok());

        let space = result.unwrap();
        let result = space.parse(&mut LinkGenerator::new());

        assert!(result.is_err());
        assert_eq!(
            format!("{}", result.err().unwrap()),
            "Error parsing space: Duplicate title 'The Same Heading' in [markdown2.md]"
        )
    }

    #[test]
    fn it_checks_page_links_exist() {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str("# Page 1\nLink to page 2: [link](subpage/markdown2.md)\n")
            .unwrap();
        temp.child("test/subpage/index.md")
            .write_str("# Subpage\n")
            .unwrap();
        temp.child("test/subpage/markdown2.md")
            .write_str("# Page 2\nLink to non-existing page: [link](does_not_exist.md)")
            .unwrap();

        let result = MarkdownSpace::from_directory(temp.child("test").path());

        assert!(result.is_ok());

        let space = result.unwrap();

        let result = space.parse(&mut LinkGenerator::new());
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.err().unwrap()),
            "Error parsing space: Missing file for link in [subpage\\markdown2.md] to [subpage\\does_not_exist.md]"
        )
    }

    #[test]
    fn it_checks_attachments_exist() {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/index.md")
            .write_str("# Page 1\nLink to image: ![link](test-image.png)\n")
            .unwrap();
        temp.child("test/test-image.png") // doesn't actually check the content, of course.
            .touch()
            .unwrap();
        temp.child("test/subpage/image.md")
            .write_str("# Page 2\nLink to non-existing image: ![link](image_does_not_exist.png)")
            .unwrap();

        let result = MarkdownSpace::from_directory(temp.child("test").path());

        assert!(result.is_ok());

        let space = result.unwrap();

        let result = space.parse(&mut LinkGenerator::new());
        assert!(result.is_err());
        assert_eq!(
            format!("{:#}", result.err().unwrap()),
            "Error parsing space: Missing file for attachment link in [subpage\\image.md] to [subpage\\image_does_not_exist.png]"
        )
    }
}
