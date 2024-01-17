use clap::builder::OsStr;
use comrak::{nodes::AstNode, Arena};
use regex::Regex;
use walkdir::WalkDir;

use crate::{
    error::{ConfluenceError, Result},
    link_generator::LinkGenerator,
    markdown_page::MarkdownPage,
    template_renderer::TemplateRenderer,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn is_valid_space_key(space_key: &str) -> bool {
    Regex::new("^[A-Za-z0-9]+$").unwrap().is_match(space_key)
}

pub struct MarkdownSpace<'a> {
    pub key: String,
    pub arena: Arena<AstNode<'a>>,
    pub markdown_pages: Vec<PathBuf>,
    pub dir: PathBuf,
}

impl<'a> MarkdownSpace<'a> {
    pub fn from_directory(dir: &Path) -> Result<Self> {
        let space_key = dir.file_name().unwrap().to_str().unwrap();
        if !is_valid_space_key(space_key) {
            return Err(ConfluenceError::generic_error(format!(
                "Invalid space directory/key '{}': can only be letters and numbers",
                space_key
            )));
        }
        let mut markdown_pages = Vec::<PathBuf>::default();
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().starts_with(dir.join("_tera")) {
                continue;
            }
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
        let mut template_renderer = TemplateRenderer::new(self)?;
        let markdown_pages: Vec<MarkdownPage> = self
            .markdown_pages
            .iter()
            .map(|markdown_page_path| {
                let content = match fs::read_to_string(markdown_page_path) {
                    Ok(c) => c,
                    Err(err) => {
                        return Err(ConfluenceError::generic_error(format!(
                            "Failed to read file {}: {}",
                            markdown_page_path.display(),
                            err
                        )))
                    }
                };
                let markdown_page = MarkdownPage::from_str(
                    markdown_page_path,
                    &content,
                    &self.arena,
                    String::from(
                        self.relative_page_path(markdown_page_path)?
                            .as_os_str()
                            .to_str()
                            .unwrap(),
                    ),
                    &mut template_renderer,
                )?;

                link_generator.register_markdown_page(&markdown_page)?;

                let missing_files: Vec<String> = markdown_page
                    .local_links
                    .iter()
                    .filter_map(|local_link| {
                        if !self.dir.join(&local_link.path).exists() {
                            Some(local_link.path.display().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                if !missing_files.is_empty() {
                    return Err(ConfluenceError::MissingFileLink {
                        source_file: markdown_page.source.clone(),
                        local_links: missing_files.join(","),
                    }
                    .into());
                };

                let missing_attachments: Vec<String> = markdown_page
                    .attachments
                    .iter()
                    .filter_map(|attachment| {
                        if !attachment.exists() {
                            Some(
                                attachment
                                    .strip_prefix(self.dir.as_path())
                                    .unwrap()
                                    .display()
                                    .to_string(),
                            )
                        } else {
                            None
                        }
                    })
                    .collect();

                if !missing_attachments.is_empty() {
                    return Err(ConfluenceError::MissingAttachmentLink {
                        source_file: markdown_page.source.clone(),
                        attachment_paths: missing_attachments.join(","),
                    }
                    .into());
                }

                Ok(markdown_page)
            })
            .filter_map(|r| r.map_err(|e| parse_errors.push(e)).ok())
            .collect();

        if !parse_errors.is_empty() {
            let error_string: String = parse_errors
                .iter()
                .map(|e| format!("{:#}", e))
                .collect::<Vec<String>>()
                .join("\n  ");
            return Err(ConfluenceError::generic_error(format!(
                "{} Error(s) parsing space:\n  {}",
                parse_errors.len(),
                &error_string
            )));
        }

        Ok(markdown_pages)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use assert_fs::fixture::{FileTouch, FileWriteStr as _, PathChild};

    use crate::link_generator::LinkGenerator;

    use super::MarkdownSpace;

    type Result = std::result::Result<(), anyhow::Error>;

    #[test]
    fn it_fails_if_space_directory_doesnt_exist() {
        let space = MarkdownSpace::from_directory(Path::new("brand_new"));

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

    #[test]
    fn it_fails_if_space_directory_is_invalid_space_key() {
        let invalid_space_key = "123-#@$@!"; // can only be letters and numbers
        let result = MarkdownSpace::from_directory(PathBuf::from(invalid_space_key).as_path());

        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            format!(
                "Invalid space directory/key '{}': can only be letters and numbers",
                invalid_space_key
            )
        )
    }

    #[test]
    fn it_skips_subpages_if_directory_contains_no_markdown_files() {
        let temp = assert_fs::TempDir::new().unwrap();
        temp.child("test/markdown1.md")
            .write_str("# Minimum Heading")
            .unwrap();
        temp.child("test/img/image.png").touch().unwrap();

        let result = MarkdownSpace::from_directory(temp.child("test").path());
        assert!(result.is_ok());
    }

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
        let result = space.parse(&mut LinkGenerator::default());

        assert!(result.is_err());
        assert!(format!("{:#}", result.err().unwrap())
            .contains("Duplicate title 'The Same Heading' in [markdown2.md]"))
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

        let result = space.parse(&mut LinkGenerator::default());
        assert!(result.is_err());
        let acutal_error = format!("{:#}", result.err().unwrap());

        assert_eq!(
            acutal_error,
            "1 Error(s) parsing space:\n  Missing file for link in [subpage\\markdown2.md] to [subpage\\does_not_exist.md]",
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

        let result = space.parse(&mut LinkGenerator::default());
        assert!(result.is_err());
        assert!(format!("{:#}", result.err().unwrap()).contains(
            "Missing file for attachment link in [subpage\\image.md] to [subpage\\image_does_not_exist.png]"
        ));
    }
}
