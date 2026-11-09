use serde::Deserialize;

use crate::api::types::LooseString;

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
