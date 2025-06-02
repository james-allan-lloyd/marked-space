use saphyr::Yaml;
use serde_json::json;

use crate::error::Result;

#[derive(Debug, Eq, PartialEq, Default)]
pub enum PageStatus {
    RoughDraft,
    InProgress,
    ReadyForReview,
    Verified,
    #[default]
    NotSet,
}

impl PageStatus {
    pub fn from_yaml(_yaml_fm: &Yaml) -> Result<Self> {
        match &_yaml_fm["status"] {
            Yaml::String(s) => match s.as_str() {
                "draft" => Ok(PageStatus::RoughDraft),
                _ => todo!(),
            },
            Yaml::BadValue => Ok(PageStatus::NotSet),
            _ => todo!(),
        }
    }

    pub fn to_property(&self) -> serde_json::Value {
        match self {
            PageStatus::RoughDraft => json!({"contentState": {"name": "Rough draft"}}),
            PageStatus::InProgress => todo!(),
            PageStatus::ReadyForReview => todo!(),
            PageStatus::Verified => todo!(),
            PageStatus::NotSet => json!(null),
        }
    }
}

#[cfg(test)]
mod test {
    use saphyr::{Hash, Yaml};

    use crate::error::TestResult;

    use super::PageStatus;

    #[test]
    fn it_returns_none_with_no_status() -> TestResult {
        let yaml = Yaml::Hash(Hash::new());
        let status = PageStatus::from_yaml(&yaml)?;
        assert_eq!(status, PageStatus::NotSet);

        let prop = status.to_property();
        assert!(prop.is_null());
        Ok(())
    }

    #[test]
    fn it_returns_rough_draft() -> TestResult {
        let mut hash = Hash::new();
        hash.insert(
            Yaml::String(String::from("status")),
            Yaml::String(String::from("draft")),
        );
        let yaml = Yaml::Hash(hash);
        let status = PageStatus::from_yaml(&yaml)?;
        assert_eq!(status, PageStatus::RoughDraft);

        // {"contentState":{"id":13500442,"color":"#ffc400","restrictionLevel":"NONE","name":"Rough draft"},"version":6,"isSpaceState":true,"isNewState":false,"setterAAID":"557058:048d7c01-8b68-440d-964f-07ce58d92aeb"}
        let prop = status.to_property();
        assert_eq!(prop["contentState"]["name"], "Rough draft");
        Ok(())
    }
}
