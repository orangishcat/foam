use crate::types::LooseString;

use super::types::{ApiLinks, LooseInt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub parent: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub inline: LooseInt,
    #[serde(default)]
    pub created: LooseInt,
    #[serde(default)]
    pub children: Vec<i64>,
    #[serde(default)]
    pub num_assignees: LooseInt,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub links: ApiLinks,
}
