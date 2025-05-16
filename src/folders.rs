use std::path::PathBuf;

use crate::{
    confluence_client::ConfluenceClient,
    confluence_space::ConfluenceSpace,
    console::{print_status, Status::Updated},
    error::Result,
    link_generator::LinkGenerator,
    markdown_page::MarkdownPage,
    parent::get_parent_file,
};

pub fn sync_folder(
    markdown_page: &MarkdownPage,
    link_generator: &LinkGenerator,
    space: &ConfluenceSpace,
    confluence_client: &ConfluenceClient,
) -> Result<()> {
    let page_id = link_generator
        .get_file_id(&PathBuf::from(&markdown_page.source))
        .expect("error: All pages should have been created already.");

    let parent_id = get_parent_file(&PathBuf::from(&markdown_page.source))
        .and_then(|f| link_generator.get_file_id(&f))
        .or(Some(space.homepage_id.clone()));

    let existing_folder = space
        .get_existing_node(&page_id)
        .expect("error: Page should have been created already.");

    if existing_folder.page_data().is_some() {
        return Err(anyhow::anyhow!("{} is a page and cannot be converted to a folder at this time. Please remove the page to create the folder.", existing_folder.title));
    }

    if existing_folder.parent_id != parent_id {
        confluence_client
            .move_page(&page_id, &parent_id.unwrap())?
            .error_for_status()?;

        print_status(
            Updated,
            &format!("[{}] {}", markdown_page.source, markdown_page.title),
        );
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use comrak::{nodes::AstNode, Arena};

    use crate::{
        confluence_page::{ConfluenceFolder, ConfluenceNode, ConfluenceNodeType},
        error::TestResult,
        link_generator::LinkGenerator,
        test_helpers::markdown_page_from_str,
    };

    #[test]
    fn it_creates_folders_when_flag_is_present() -> TestResult {
        let mut link_generator = LinkGenerator::default();

        let new_title = String::from("New Title");
        let new_source = String::from("new-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &new_source,
            &format!("---\nfolder: true\n---\n# {} \n", new_title),
            &arena,
        )?)?;

        let pages_to_create = link_generator.get_nodes_to_create();

        assert_eq!(pages_to_create, vec![new_title.clone()]);
        assert!(link_generator.is_folder(&new_title));

        Ok(())
    }

    #[test]
    fn it_does_not_create_folders_if_they_already_exist() -> TestResult {
        let mut link_generator = LinkGenerator::default();

        let existing_title = String::from("Existing Title");
        let existing_source = String::from("existing-test.md");

        let arena = Arena::<AstNode>::new();
        link_generator.register_markdown_page(&markdown_page_from_str(
            &existing_source,
            &format!("---\nfolder: true\n---\n# {} \n", existing_title),
            &arena,
        )?)?;

        link_generator.register_confluence_node(&ConfluenceNode {
            id: "1".into(),
            title: existing_title.clone(),
            parent_id: Some("99".into()),
            data: ConfluenceNodeType::Folder(ConfluenceFolder {}),
        });

        let pages_to_create = link_generator.get_nodes_to_create();

        assert_eq!(pages_to_create.len(), 0);
        assert!(link_generator.is_folder(&existing_title));

        Ok(())
    }

    // #[test]
    // fn it_warns_if_page_has_content_with_flag() {
    //     todo!();
    // }
    //
    // #[test]
    // fn it_converts_pages_to_folders() {
    //     todo!();
    // }
}
