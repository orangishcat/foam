use crate::types::LooseString;

use super::types::LooseInt;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Package {
    pub id: LooseString,
    pub title: String,
    pub url: String,
    pub num_attempts: LooseInt,
    pub scorm_grading_enabled: LooseInt,
    pub sco_grading_enabled: LooseInt,
    pub grade_timing_type: LooseInt,
    pub grade_timing_option: LooseInt,
    pub available: LooseInt,
    pub completed: LooseInt,
    pub completion_status: String,
    pub count_in_grade: LooseInt,
}
