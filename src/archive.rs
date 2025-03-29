use std::path::Path;

use crate::{
    confluence_client::ConfluenceClient,
    confluence_page::ConfluencePage,
    console::{print_status, Status},
    link_generator::LinkGenerator,
    responses::ContentStatus,
};

pub(crate) fn should_archive(p: &ConfluencePage, link_generator: &LinkGenerator) -> bool {
    !matches!(&p.status, ContentStatus::Archived) && link_generator.is_orphaned(p)
}

pub(crate) fn should_unarchive(p: &ConfluencePage, link_generator: &LinkGenerator) -> bool {
    matches!(&p.status, ContentStatus::Archived) && !link_generator.is_orphaned(p)
}

pub(crate) fn unarchive(
    p: &ConfluencePage,
    confluence_client: &ConfluenceClient,
) -> anyhow::Result<()> {
    let path = p.path.clone();
    print_status(
        Status::Unarchived,
        &format!(
            "restored \"{}\" from {}",
            p.title,
            path.unwrap_or_default().display()
        ),
    );
    p.unarchive(confluence_client)
}

pub(crate) fn archive(
    p: &ConfluencePage,
    space_dir: &Path,
    confluence_client: &ConfluenceClient,
) -> anyhow::Result<()> {
    if let Some(path) = &p.path {
        if !space_dir.join(path).exists() {
            print_status(
                Status::Archived,
                &format!(
                    "orphaned \"{}\" from {} (deleted)",
                    p.title,
                    space_dir.join(path).display()
                ),
            );
        }
    } else {
        print_status(
            Status::Archived,
            &format!(
                "orphaned page \"{}\" (probably created outside of markedspace)",
                p.title
            ),
        );
    }

    p.archive(confluence_client)
}

#[cfg(test)]
mod tests {
    use comrak::{nodes::AstNode, Arena};

    use crate::{
        archive::{should_archive, should_unarchive},
        confluence_page::ConfluencePage,
        error::TestResult,
        link_generator::LinkGenerator,
        responses::{ContentStatus, Version},
        test_helpers::markdown_page_from_str,
    };

    fn nonorphan(status: ContentStatus) -> ConfluencePage {
        ConfluencePage {
            id: String::from("1"),
            title: String::from("Not Orphaned Page"),
            parent_id: None,
            version: Version {
                message: String::from(ConfluencePage::version_message_prefix()),
                number: 1,
            },
            path: None, // "foo.md".to_string(),
            status,
        }
    }

    fn orphan(status: ContentStatus) -> ConfluencePage {
        ConfluencePage {
            id: String::from("2"),
            title: String::from("Orphaned Page"),
            parent_id: None,
            version: Version {
                message: String::from(ConfluencePage::version_message_prefix()),
                number: 1,
            },
            path: None, // "foo.md".to_string(),
            status,
        }
    }

    fn test_link_generator() -> LinkGenerator {
        LinkGenerator::new("example.atlassian.net", "TEST")
    }

    #[test]
    fn it_archives_orphans() {
        assert!(should_archive(
            &orphan(ContentStatus::Current),
            &test_link_generator()
        ));
    }

    #[test]
    fn it_unarchives_non_orphans() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let mut link_generator = test_link_generator();
        let page = nonorphan(ContentStatus::Archived);
        link_generator.register_markdown_page(&markdown_page_from_str(
            "test.md",
            &format!("# {} \n", page.title),
            &arena,
        )?)?;

        assert!(!link_generator.is_orphaned(&page));
        assert!(should_unarchive(&page, &link_generator));

        Ok(())
    }

    #[test]
    fn it_does_not_archive_nonorphans() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let mut link_generator = test_link_generator();
        let page = nonorphan(ContentStatus::Current);
        link_generator.register_markdown_page(&markdown_page_from_str(
            "test.md",
            &format!("# {} \n", page.title),
            &arena,
        )?)?;

        assert!(!link_generator.is_orphaned(&page));
        assert!(!should_archive(&page, &link_generator));

        Ok(())
    }

    #[test]
    fn it_does_not_unarchive_orphans() {
        assert!(!should_unarchive(
            &orphan(ContentStatus::Archived),
            &test_link_generator()
        ));
    }
}
