use crate::types::LooseString;

use super::types::LooseInt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub num_attempts: LooseInt,
    #[serde(default)]
    pub scorm_grading_enabled: LooseInt,
    #[serde(default)]
    pub sco_grading_enabled: LooseInt,
    #[serde(default)]
    pub grade_timing_type: LooseInt,
    #[serde(default)]
    pub grade_timing_option: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub count_in_grade: LooseInt,
}
