use clap::builder::OsStr;
use comrak::{nodes::AstNode, Arena};
use regex::Regex;
use walkdir::WalkDir;

use crate::{
    console::{print_info, print_warning},
    error::{ConfluenceError, Result},
    markdown_page::MarkdownPage,
    template_renderer::TemplateRenderer,
};
use std::{
    collections::HashSet,
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
    #[cfg(test)]
    pub fn default(key: &str, dir: &Path) -> Self {
        MarkdownSpace {
            markdown_pages: Vec::default(),
            key: String::from(key),
            dir: PathBuf::from(dir),
            arena: Arena::new(),
        }
    }

    #[cfg(test)]
    pub fn page_from_str(
        &'a self,
        filename: &str,
        content: &str,
    ) -> crate::error::Result<MarkdownPage<'a>> {
        MarkdownPage::from_str(
            &PathBuf::from(filename),
            content,
            &self.arena,
            filename.to_string(),
            &mut TemplateRenderer::default()?,
        )
    }

    pub fn from_directory(dir: &Path) -> Result<Self> {
        let space_key = dir.file_name().unwrap().to_str().unwrap();
        if !is_valid_space_key(space_key) {
            return Err(ConfluenceError::generic_error(format!(
                "Invalid space directory/key '{}': can only be letters and numbers",
                space_key
            )));
        }
        print_info(&format!(
            "Parsing space {} from {} ...",
            space_key,
            dir.display()
        ));
        let mut markdown_pages = Vec::<PathBuf>::default();
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().starts_with(dir.join("_tera")) {
                continue;
            }
            if entry.path().is_dir() {
                if !entry.path().join("index.md").exists() {
                    print_warning(&format!(
                        "directory {} is missing index.md",
                        entry.path().display()
                    ));
                }
            } else if entry.path().extension() == Some(&OsStr::from("md")) {
                markdown_pages.push(entry.into_path());
            }
        }
        let key = String::from(dir.file_stem().unwrap().to_str().unwrap());
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

    pub fn space_relative_path_string(&self, page_path: &Path) -> Result<String> {
        let space_relative_path = page_path.strip_prefix(&self.dir).map_err(|_e| {
            ConfluenceError::generic_error(format!(
                "Page is not in space directory {}: {} - {}",
                self.dir.display(),
                page_path.display(),
                _e
            ))
        })?;

        Ok(PathBuf::from(space_relative_path)
            .to_str()
            .ok_or(ConfluenceError::generic_error(
                "Failed to convert path to str",
            ))?
            .replace('\\', "/"))
    }

    pub(crate) fn parse(
        &'a mut self,
        template_renderer: &mut TemplateRenderer,
    ) -> Result<Vec<MarkdownPage<'a>>> {
        let mut parse_errors = Vec::<anyhow::Error>::default();
        let mut titles: HashSet<String> = HashSet::default();
        let markdown_pages: Vec<MarkdownPage> = self
            .markdown_pages
            .iter()
            .map(|markdown_page_path| {
                let markdown_page = MarkdownPage::from_file(
                    &self.dir,
                    markdown_page_path,
                    &self.arena,
                    template_renderer,
                )?;

                for warning in markdown_page.warnings.iter() {
                    print_warning(warning);
                }
                let title = markdown_page.title.to_owned();
                let filename = markdown_page.source.replace('\\', "/");
                if titles.contains(&title) {
                    return Err(ConfluenceError::DuplicateTitle {
                        file: filename,
                        title,
                    }
                    .into());
                }
                titles.insert(title.clone());

                // if markdown_page.is_folder() {
                //     self.folders.insert(title.clone());
                // }
                // file_to_title.insert(filename.clone(), title.clone());

                let missing_files: Vec<String> = markdown_page
                    .local_links
                    .iter()
                    .filter_map(|local_link| {
                        if !local_link.target.exists() {
                            Some(local_link.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                if !missing_files.is_empty() {
                    return Err(ConfluenceError::MissingFileLink {
                        source_file: markdown_page.source.clone(),
                        local_links: missing_files
                            .iter()
                            .map(|l| self.space_relative_path_string(&PathBuf::from(l)).unwrap())
                            .collect::<Vec<String>>()
                            .join(","),
                    }
                    .into());
                };

                let missing_attachments: Vec<String> = markdown_page
                    .attachments
                    .iter()
                    .filter_map(|attachment| {
                        if !attachment.link.target.exists() {
                            Some(
                                self.space_relative_path_string(&attachment.link.target)
                                    .unwrap(),
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
    use anyhow::{anyhow, Ok};
    use std::path::{Path, PathBuf};

    use assert_fs::fixture::{FileTouch, FileWriteStr as _, PathChild};

    use crate::{
        attachments::Attachment, error::TestResult, local_link::LocalLink,
        markdown_page::MarkdownPage, template_renderer::TemplateRenderer,
    };

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
        assert_eq!(result?.key, "test");

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

        let mut space = result.unwrap();
        let result = parse_default(&mut space);

        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(format!("{:#}", error).contains("Duplicate title 'The Same Heading' in [markdown"))
    }

    #[test]
    fn it_checks_page_links_exist() -> TestResult {
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

        let mut space = result.unwrap();

        let result = parse_default(&mut space);
        assert!(result.is_err());
        let acutal_error = format!("{:#}", result.err().unwrap());

        assert_eq!(
            acutal_error,
            "1 Error(s) parsing space:\n  Missing file for link in [subpage/markdown2.md] to [subpage/does_not_exist.md]",
        );

        Ok(())
    }

    fn parse_default<'a>(
        space: &'a mut MarkdownSpace<'a>,
    ) -> anyhow::Result<Vec<MarkdownPage<'a>>, anyhow::Error> {
        space.parse(&mut TemplateRenderer::default()?)
    }

    #[test]
    fn it_checks_attachments_exist() -> TestResult {
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

        let mut space = result.unwrap();

        let result = parse_default(&mut space);
        let acutal_error = format!("{:#}", result.err().unwrap());
        assert!(acutal_error.contains(
            "Missing file for attachment link in [subpage/image.md] to [subpage/image_does_not_exist.png]"
        ));
        Ok(())
    }

    #[test]
    fn it_links_attachments_in_subdirs_without_index_md() -> TestResult {
        let temp = assert_fs::TempDir::new()?;
        temp.child("test/index.md").write_str("# Space Index")?;
        let temp_markdown = temp.child("test/subdir/index.md");
        temp_markdown.write_str("# Page 1\nLink to image: ![Data Model](assets/image.png)\n")?;
        let temp_image = temp.child("test/subdir/assets/image.png"); // doesn't actually check the content, of course.
        temp_image.touch()?;

        let result = MarkdownSpace::from_directory(temp.child("test").path());

        assert!(result.is_ok());

        let mut space = result.unwrap();

        let result = parse_default(&mut space)?;

        let page = &result
            .iter()
            .find(|x| x.title == "Page 1")
            .ok_or(anyhow!("No page"))?;

        assert_eq!(page.warnings, Vec::<String>::default());

        assert_eq!(
            page.attachments,
            vec![Attachment::image(LocalLink::from_str(
                "assets/image.png",
                temp_markdown.path().parent().unwrap()
            )?)]
        );

        Ok(())
    }

    #[test]
    fn it_links_non_image_attachments() -> TestResult {
        let temp = assert_fs::TempDir::new()?;
        let test_markdown = temp.child("test/index.md");
        test_markdown
            .write_str("# Page 1\nLink to text file : [Some Text File](./some-text-file.txt)\n")?;
        temp.child("test/some-text-file.txt").touch()?;

        let mut space =
            MarkdownSpace::from_directory(temp.child("test").path()).expect("Space loads");

        let result = parse_default(&mut space)?;

        let page = &result
            .iter()
            .find(|x| x.title == "Page 1")
            .ok_or(anyhow!("Expected our page to parse, but didn't find it"))?;

        assert_eq!(page.warnings, Vec::<String>::default());

        let local_link = LocalLink::from_str(
            "./some-text-file.txt",
            test_markdown.path().parent().unwrap(),
        )?;

        assert_eq!(page.attachments, vec![Attachment::file(local_link)]);

        Ok(())
    }
}
