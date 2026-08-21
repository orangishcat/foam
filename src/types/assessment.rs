use crate::types::{LooseFloat, LooseInt, loose_str::LooseString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub max_points: LooseFloat,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub grading_scale: LooseInt,
    #[serde(default)]
    pub grading_period: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
}
