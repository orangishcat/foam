use serde::{Deserialize, Serialize};

use crate::types::LooseString;

#[derive(Serialize, oauth::Request)]
pub(crate) struct GradesQuery<'a> {
    pub section_id: &'a str,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(crate) struct GradesResponse {
    pub section: Vec<SectionGrades>,
}

#[derive(Deserialize)]
pub(crate) struct SectionGrades {
    pub section_id: LooseString,
    #[serde(default)]
    pub period: Vec<PeriodGrades>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(crate) struct PeriodGrades {
    pub assignment: Vec<AssignmentGrade>,
}

#[derive(Deserialize)]
pub(crate) struct AssignmentGrade {
    pub assignment_id: LooseString,
    // Scores may be numeric or grading-scale letters.
    pub grade: Option<LooseString>,
}
