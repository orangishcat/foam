use super::types::{ApiLinks, LooseInt, string};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discussion {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub uid: LooseInt,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub weight: LooseInt,
    #[serde(default)]
    pub graded: LooseInt,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub grade_item_id: LooseInt,
    #[serde(default)]
    pub grading_scale: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
    pub auto_publish_grades: LooseInt,
    #[serde(default)]
    pub comments_closed: LooseInt,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub links: ApiLinks,
}
