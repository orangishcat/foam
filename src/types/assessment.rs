use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assessment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub max_points: f64,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub grading_scale: i64,
    #[serde(default)]
    pub grading_period: i64,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub completed: bool,
}
