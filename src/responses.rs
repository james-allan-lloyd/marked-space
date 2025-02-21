use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub message: String,
    pub number: i32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BodyType {
    pub representation: String,
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub enum BodyBulk {
    #[serde(rename = "storage")]
    Storage(BodyType),
    #[serde(rename = "atlas_doc_format")]
    AtlasDocFormat(BodyType),
    #[serde(other)]
    Empty,
}

// TODO: might be a better way to express this...
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageBulkWithoutBody {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub version: Version,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageBulk {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub version: Version,
    pub body: BodyBulk,
}

#[derive(Deserialize, Debug)]
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
pub struct PageSingleWithoutBody {
    pub id: String,
    pub title: String,
    pub version: Version,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
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
pub struct Space {
    pub id: String,
    pub key: String,
    pub _name: String,
    pub homepage_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub title: String,
    pub page_id: String,
    pub comment: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
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
