use super::types::{LooseInt, string};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
    pub auto_publish_grades: LooseInt,
}
