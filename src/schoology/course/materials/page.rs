use crate::types::LooseString;

use super::types::{ApiLinks, LooseInt};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub id: LooseString,
    pub title: String,
    pub body: String,
    pub parent: LooseInt,
    pub published: LooseInt,
    pub inline: LooseInt,
    pub created: LooseInt,
    pub children: Vec<i64>,
    pub num_assignees: LooseInt,
    pub assignees: Vec<i64>,
    pub grading_group_ids: Vec<i64>,
    pub available: LooseInt,
    pub completed: LooseInt,
    pub completion_status: String,
    pub links: ApiLinks,
}
