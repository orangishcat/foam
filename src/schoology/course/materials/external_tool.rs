use crate::types::LooseString;

use super::types::LooseInt;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalTool {
    pub id: LooseString,
    pub title: String,
    pub url: String,
    pub description: String,
    pub available: LooseInt,
    pub published: LooseInt,
    pub count_in_grade: LooseInt,
    pub collected_only: LooseInt,
    pub auto_publish_grades: LooseInt,
}
