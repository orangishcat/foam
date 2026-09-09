use crate::{schoology::types::datetime::SchoologyDatetime, types::LooseString};

use super::types::{ApiLinks, LooseInt};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Discussion {
    pub id: LooseString,
    pub uid: LooseInt,
    pub title: String,
    pub body: String,
    pub weight: LooseInt,
    pub graded: LooseInt,
    pub due: SchoologyDatetime,
    pub grade_item_id: LooseInt,
    pub grading_scale: LooseInt,
    pub published: LooseInt,
    pub available: LooseInt,
    pub completed: LooseInt,
    pub count_in_grade: LooseInt,
    pub collected_only: LooseInt,
    pub auto_publish_grades: LooseInt,
    pub comments_closed: LooseInt,
    pub completion_status: String,
    pub links: ApiLinks,
}
