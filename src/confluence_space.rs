use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::archive::{archive, should_archive, should_unarchive, unarchive};
use crate::confluence_client::ConfluenceClient;
use crate::confluence_page::{ConfluenceNode, ConfluenceNodeType, ConfluencePageData};
use crate::console::{print_status, Status};
use crate::error::{self, ConfluenceError};
use crate::link_generator::LinkGenerator;

use crate::page_statuses::ContentStates;
use crate::responses::{self, ContentStatus, PageSingleWithoutBody, Version};
use crate::sync_operation::SyncOperation;

#[derive(Debug)]
pub struct ConfluenceSpace {
    pub id: String,
    pub homepage_id: String,
    nodes: Vec<ConfluenceNode>,
    pub content_states: ContentStates,
}

impl ConfluenceSpace {
    pub fn get(confluence_client: &ConfluenceClient, space_key: &str) -> Result<ConfluenceSpace> {
        let resp = confluence_client.get_space_by_key(space_key)?;
        if !resp.status().is_success() {
            return Err(ConfluenceError::failed_request(resp));
        }

        let json = resp.json::<serde_json::Value>()?;

        if json["results"].as_array().unwrap().is_empty() {
            return Err(ConfluenceError::generic_error(format!(
                "No such space: {}",
                space_key
            )));
        }

        let parsed_space = serde_json::from_value::<responses::Space>(json["results"][0].clone())?;

        let parsed_content_states = confluence_client
            .get_space_suggested_content_states(space_key)?
            .error_for_status()?
            .json::<Vec<responses::ContentState>>()?;

        let content_states = ContentStates::new(&parsed_content_states);

        Ok(ConfluenceSpace {
            id: parsed_space.id,
            homepage_id: parsed_space.homepage_id,
            nodes: Vec::default(),
            content_states,
        })
    }

    pub fn read_all_pages(&mut self, confluence_client: &ConfluenceClient) -> Result<()> {
        self.nodes = ConfluenceNode::get_all(confluence_client, self)?;
        Ok(())
    }

    pub fn link_pages(&mut self, link_generator: &mut LinkGenerator) {
        self.nodes.iter().for_each(|confluence_page| {
            link_generator.register_confluence_node(confluence_page);
        });
    }

    pub(crate) fn restore_archived_pages(
        &self,
        link_generator: &LinkGenerator,
        confluence_client: &ConfluenceClient,
    ) -> anyhow::Result<()> {
        let _errors = self
            .nodes
            .iter()
            .filter(|p| should_unarchive(p, link_generator))
            .filter_map(|p| unarchive(p, confluence_client).err())
            .collect::<Vec<anyhow::Error>>();

        Ok(())
    }

    pub(crate) fn archive_orphans(
        &self,
        link_generator: &LinkGenerator,
        space_dir: &Path,
        confluence_client: &ConfluenceClient,
    ) -> error::Result<()> {
        // let orphaned_pages = self.get_orphans(link_generator);
        let _errors = self
            .nodes
            .iter()
            .filter(|p| should_archive(p, link_generator))
            .filter_map(|p| archive(p, space_dir, confluence_client).err())
            .collect::<Vec<anyhow::Error>>();

        Ok(())
    }

    pub fn get_existing_node(&self, node_id: &str) -> Option<ConfluenceNode> {
        self.nodes.iter().find(|node| node.id == node_id).cloned()
    }

    pub fn add_node(&mut self, from: ConfluenceNode) {
        self.nodes.push(from);
    }

    pub fn create_initial_nodes(
        &mut self,
        link_generator: &mut LinkGenerator,
        confluence_client: &ConfluenceClient,
    ) -> Result<()> {
        for title in link_generator.get_nodes_to_create() {
            if link_generator.is_folder(&title) {
                self.create_folder(title, confluence_client, link_generator)?;
            } else {
                self.create_page(title, confluence_client, link_generator)?;
            }
        }
        Ok(())
    }

    fn create_page(
        &mut self,
        title: String,
        confluence_client: &ConfluenceClient,
        link_generator: &mut LinkGenerator,
    ) -> Result<(), anyhow::Error> {
        let op = SyncOperation::start(format!("Creating new page \"{}\"", title), true);
        let resp = confluence_client.create_page(json!({
            "spaceId": self.id,
            "status": "current",
            "title": title,
            "parentId": self.homepage_id.clone(),
        }))?;
        if !resp.status().is_success() {
            op.end(Status::Error);
            return Err(ConfluenceError::failed_request(resp));
        }
        let page: PageSingleWithoutBody = resp.json()?;
        let existing_page = ConfluenceNode {
            id: page.id,
            title: title.clone(),
            parent_id: Some(self.homepage_id.clone()),
            data: ConfluenceNodeType::Page(ConfluencePageData {
                version: Version {
                    number: 1,
                    message: String::default(),
                },
                path: None,
                status: ContentStatus::Current,
            }),
        };
        link_generator.register_confluence_node(&existing_page);
        self.add_node(existing_page);
        op.end(Status::Created);
        Ok(())
    }

    fn create_folder(
        &self,
        title: String,
        confluence_client: &ConfluenceClient,
        _link_generator: &mut LinkGenerator,
    ) -> Result<(), anyhow::Error> {
        confluence_client
            .create_folder(json!({
                "spaceId": self.id,
                "title": title,
                "parent_id": self.homepage_id.clone()
            }))?
            .error_for_status()?;

        print_status(Status::Created, &format!("folder \"{}\"", title));
        Ok(())
    }
}
