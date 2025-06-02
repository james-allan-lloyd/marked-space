use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub message: String,
    pub number: i32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BodyType {
    pub representation: String,
    pub value: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub enum BodyBulk {
    #[serde(rename = "storage")]
    Storage(BodyType),
    #[serde(rename = "atlas_doc_format")]
    AtlasDocFormat(BodyType),
    #[serde(other)]
    Empty,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ContentStatus {
    Current,
    Draft,
    Archived,
    Historical,
    Trashed,
    Deleted,
    Any,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContentState {
    pub id: u64,
    pub name: String,
    pub color: String,
}

// TODO: might be a better way to express this...
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageBulkWithoutBody {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub status: ContentStatus,
    pub version: Version,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PageBulk {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub version: Version,
    pub body: BodyBulk,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub enum BodySingle {
    #[serde(rename = "storage")]
    Storage(BodyType),
    #[serde(rename = "atlas_doc_format")]
    AtlasDocFormat(BodyType),
    #[serde(rename = "view")]
    View(BodyType),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PageSingleWithoutBody {
    pub id: String,
    pub title: String,
    pub version: Version,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PageSingleWithBody {
    pub id: String,
    pub title: String,
    pub version: Version,
    pub body: BodySingle,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub next: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MultiEntityResult<T> {
    pub results: Vec<T>,
    #[serde(rename = "_links")]
    pub links: Option<Links>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Space {
    pub id: String,
    pub key: String,
    pub _name: String,
    pub homepage_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Attachment {
    pub id: String,
    pub title: String,
    pub page_id: String,
    pub comment: String,
    pub file_id: String, // File ID of the attachment. This is the ID referenced in atlas_doc_format bodies and is distinct from the attachment ID.
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Label {
    pub prefix: String,
    pub name: String,
    pub id: String,
    pub label: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentProperty {
    pub id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub version: Version,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub user: User,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(rename = "type")]
    pub _type: String,
    pub account_id: String,
    // pub email: String,
    // pub public_name: String,
    // pub display_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Descendant {
    pub id: String,
    // pub status: OnlyArchivedAndCurrentContentStatus
    pub title: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub parent_id: String,
    // pub depth: i32,
    // pub child_position: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub id: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub status: String,
    pub title: String,
    pub extensions: serde_json::Value,
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    #[test]
    fn it_handles_missing_content_in_bulk() {
        let page = json!( {
            "parentType": "page",
            "createdAt": "2023-12-12T07:25:30.563Z",
            "authorId": "557058:048d7c01-8b68-440d-964f-07ce58d92aeb",
            "id": "7700526",
            "version": {
                "number": 1,
                "message": "",
                "minorEdit": false,
                "authorId": "557058:048d7c01-8b68-440d-964f-07ce58d92aeb",
                "createdAt": "2023-12-12T07:25:30.563Z"
            },
            "position": 3997,
            "title": "A page with sub pages",
            "status": "current",
            "ownerId": "557058:048d7c01-8b68-440d-964f-07ce58d92aeb",
            "body": {},
            "parentId": "98307",
            "spaceId": "98306",
            "lastOwnerId": null,
            "_links": {
                "editui": "/pages/resumedraft.action?draftId=7700526",
                "webui": "/spaces/TEAM/pages/7700526/A+page+with+sub+pages",
                "tinyui": "/x/LoB1"
            }
        });

        let result = serde_json::from_value::<PageBulkWithoutBody>(page);

        assert!(result.is_ok());
    }
}
