use std::collections::HashMap;

use once_cell::sync::OnceCell;
use saphyr::Yaml;
use serde_json::json;

use crate::{error::Result, responses};

#[derive(Debug, Eq, PartialEq, Default, Hash)]
pub enum PageStatus {
    RoughDraft,
    InProgress,
    ReadyForReview,
    Verified,
    #[default]
    NotSet,
}

static SPACE_CONTENT_STATES: OnceCell<HashMap<PageStatus, responses::ContentState>> =
    OnceCell::new();

impl PageStatus {
    pub fn from_yaml(_yaml_fm: &Yaml) -> Result<Self> {
        match _yaml_fm {
            Yaml::String(s) => match s.as_str() {
                "draft" => Ok(PageStatus::RoughDraft),
                _ => todo!(),
            },
            Yaml::BadValue => Ok(PageStatus::NotSet),
            _ => todo!(),
        }
    }

    pub fn to_property(&self) -> serde_json::Value {
        if let Some(content_state) = SPACE_CONTENT_STATES
            .get()
            .and_then(|hash_map| hash_map.get(self))
        {
            json!({"contentState": content_state})
        } else {
            json!(null)
        }
    }

    pub fn set_space_content_states(content_states: &[responses::ContentState]) {
        let standard_states = vec![
            (PageStatus::RoughDraft, "Rough draft"),
            (PageStatus::InProgress, "In progress"),
            (PageStatus::ReadyForReview, "Ready for review"),
            (PageStatus::Verified, "Verified"),
        ];
        let mut hash_map = HashMap::new();
        for (status, status_name) in standard_states {
            if let Some(content_state) = content_states.iter().find(|x| x.name == status_name) {
                hash_map.insert(status, content_state.clone());
            }
        }
        SPACE_CONTENT_STATES
            .set(hash_map)
            .expect("Should be able to set the content states");
    }
}

#[cfg(test)]
mod test {
    use saphyr::Yaml;
    use serde_json::json;

    use crate::{error::TestResult, responses};

    use super::PageStatus;

    #[test]
    fn it_returns_none_with_no_status() -> TestResult {
        let status = PageStatus::from_yaml(&Yaml::BadValue)?;
        assert_eq!(status, PageStatus::NotSet);

        let prop = status.to_property();
        assert!(prop.is_null());
        Ok(())
    }

    #[test]
    fn it_returns_rough_draft() -> TestResult {
        let response = json!([{"id":13500442,"color":"#ffc400","name":"Rough draft"}]); // ,{"id":13500443,"color":"#2684ff","name":"In progress"},{"id":13500444,"color":"#57d9a3","name":"Ready for review"},{"id":37912577,"color":"#1d7afc","name":"Verified"}]);
        let states = serde_json::from_value::<Vec<responses::ContentState>>(response).unwrap();
        PageStatus::set_space_content_states(&states);
        let status = PageStatus::from_yaml(&Yaml::String(String::from("draft")))?;
        assert_eq!(status, PageStatus::RoughDraft);

        // {"contentState":{"id":13500442,"color":"#ffc400","restrictionLevel":"NONE","name":"Rough draft"},"version":6,"isSpaceState":true,"isNewState":false,"setterAAID":"557058:048d7c01-8b68-440d-964f-07ce58d92aeb"}
        let prop = status.to_property();
        assert_eq!(prop["contentState"]["name"], states[0].name);
        assert_eq!(prop["contentState"]["id"], states[0].id);
        assert_eq!(prop["contentState"]["color"], states[0].color);
        Ok(())
    }
}
