use std::path::Path;

use crate::{
    confluence_client::ConfluenceClient,
    confluence_page::{ConfluenceNode, ConfluenceNodeType},
    console::{print_status, Status},
    link_generator::LinkGenerator,
    responses::ContentStatus,
};

pub(crate) fn should_archive(node: &ConfluenceNode, link_generator: &LinkGenerator) -> bool {
    match &node.data {
        ConfluenceNodeType::Page(p) => {
            !matches!(&p.status, ContentStatus::Archived)
                && link_generator.is_orphaned(node, p)
                && p.is_managed()
        }
        ConfluenceNodeType::Folder(_confluence_folder) => false,
    }
}

pub(crate) fn should_unarchive(node: &ConfluenceNode, link_generator: &LinkGenerator) -> bool {
    match &node.data {
        ConfluenceNodeType::Page(p) => {
            matches!(&p.status, ContentStatus::Archived)
                && !link_generator.is_orphaned(node, p)
                && p.is_managed()
        }
        ConfluenceNodeType::Folder(_confluence_folder) => false,
    }
}

pub(crate) fn unarchive(
    node: &ConfluenceNode,
    confluence_client: &ConfluenceClient,
) -> anyhow::Result<()> {
    match &node.data {
        crate::confluence_page::ConfluenceNodeType::Page(p) => {
            let path = p.path.clone();
            print_status(
                Status::Unarchived,
                &format!(
                    "restored \"{}\" from {}",
                    node.title,
                    path.unwrap_or_default().display()
                ),
            );
            node.unarchive(confluence_client)
        }
        crate::confluence_page::ConfluenceNodeType::Folder(_confluence_folder) => todo!(),
    }
}

pub(crate) fn archive(
    node: &ConfluenceNode,
    space_dir: &Path,
    confluence_client: &ConfluenceClient,
) -> anyhow::Result<()> {
    match &node.data {
        crate::confluence_page::ConfluenceNodeType::Page(p) => {
            if let Some(path) = &p.path {
                if !space_dir.join(path).exists() {
                    print_status(
                        Status::Archived,
                        &format!(
                            "orphaned \"{}\" from {} (deleted)",
                            node.title,
                            space_dir.join(path).display()
                        ),
                    );
                }
            } else {
                print_status(
                    Status::Archived,
                    &format!(
                        "orphaned page \"{}\" (probably created outside of markedspace)",
                        node.title
                    ),
                );
            }

            node.archive(confluence_client)
        }
        crate::confluence_page::ConfluenceNodeType::Folder(_confluence_folder) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use comrak::{nodes::AstNode, Arena};

    use crate::{
        archive::{should_archive, should_unarchive},
        confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData},
        error::TestResult,
        link_generator::LinkGenerator,
        responses::{ContentStatus, Version},
        test_helpers::markdown_page_from_str,
    };

    fn nonorphan(status: ContentStatus) -> ConfluenceNode {
        ConfluenceNode {
            id: String::from("1"),
            title: String::from("Not Orphaned Page"),
            parent_id: None,

            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::from(ConfluencePageData::version_message_prefix()),
                    number: 1,
                },
                path: Some("test.md".into()),
                status,
            }),
        }
    }

    fn orphan(status: ContentStatus) -> ConfluenceNode {
        ConfluenceNode {
            id: String::from("2"),
            title: String::from("Orphaned Page"),
            parent_id: None,

            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: String::from(ConfluencePageData::version_message_prefix()),
                    number: 1,
                },
                path: None, // "foo.md".to_string(),
                status,
            }),
        }
    }

    fn unmanaged(status: ContentStatus) -> ConfluenceNode {
        ConfluenceNode {
            id: String::from("3"),
            title: String::from("Unmanaged Page"),
            parent_id: None,

            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    message: "".into(),
                    number: 1,
                },
                path: None, // "foo.md".to_string(),
                status,
            }),
        }
    }

    fn test_link_generator() -> LinkGenerator {
        LinkGenerator::default_test()
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

        assert!(!link_generator.is_orphaned(&page, page.page_data().unwrap()));
        assert!(should_unarchive(&page, &link_generator));

        Ok(())
    }

    #[test]
    fn it_does_not_archive_nonorphans() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let mut link_generator = test_link_generator();
        link_generator.register_markdown_page(&markdown_page_from_str(
            "test.md",
            &format!("# {} \n", "Not Orphaned Page"),
            &arena,
        )?)?;
        let node = nonorphan(ContentStatus::Current);
        link_generator.register_confluence_node(&node);

        assert!(!link_generator.is_orphaned(&node, node.page_data().unwrap()));
        assert!(!should_archive(&node, &link_generator));

        Ok(())
    }

    #[test]
    fn it_does_not_archive_retitled_pages() -> TestResult {
        let arena = Arena::<AstNode>::new();
        let mut link_generator = test_link_generator();
        let title = "Not Orphaned Page";
        link_generator.register_markdown_page(&markdown_page_from_str(
            "test.md",
            &format!("# {} Retitled\n", title),
            &arena,
        )?)?;
        let node = nonorphan(ContentStatus::Current);
        link_generator.register_confluence_node(&node);

        assert!(!link_generator.is_orphaned(&node, node.page_data().unwrap()));
        assert!(!should_archive(&node, &link_generator));

        Ok(())
    }

    #[test]
    fn it_does_not_unarchive_orphans() {
        assert!(!should_unarchive(
            &orphan(ContentStatus::Archived),
            &test_link_generator()
        ));
    }

    #[test]
    fn it_does_not_unarchive_unmanaged() {
        assert!(!should_unarchive(
            &unmanaged(ContentStatus::Archived),
            &test_link_generator()
        ));
    }

    #[test]
    fn it_does_not_archive_unmanaged() {
        assert!(!should_archive(
            &unmanaged(ContentStatus::Current),
            &test_link_generator()
        ));
    }
}
