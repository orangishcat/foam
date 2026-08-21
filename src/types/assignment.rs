use crate::types::{LooseInt, loose_str::LooseString};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due: DateTime<Utc>,
    #[serde(default)]
    pub max_points: f32,
    #[serde(default)]
    pub allow_submissions: bool,
    #[serde(default)]
    pub show_rubric: bool,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
    pub auto_publish_grades: LooseInt,
}
