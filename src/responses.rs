use serde::Deserialize;

#[derive(Deserialize, Debug)]
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
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageBulk {
    pub id: String,
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
pub struct PageSingle {
    pub id: String,
    pub title: String,
    pub version: Version,
    pub body: BodySingle,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MultiEntityResult<T> {
    pub results: Vec<T>,
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
