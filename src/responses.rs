use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub enum BodyBulk {
    #[serde(rename = "storage")]
    Storage {
        representation: String,
        value: String,
    },
    #[serde(rename = "atlas_doc_format")]
    AtlasDocFormat {
        representation: String,
        value: String,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub message: String,
    pub number: i32,
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
#[serde(rename_all = "camelCase")]
pub struct MultiEntityResult {
    pub results: Vec<PageBulk>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub _key: String,
    pub _name: String,
    pub homepage_id: String,
}
