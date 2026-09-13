use crate::types::material::MaterialType;

pub(super) fn parse(value: &str) -> Option<MaterialType> {
    match value {
        "s_content_grade_item" => Some(MaterialType::Assignment),
        value => match value.strip_prefix("s_course_").unwrap_or(value) {
            "folder" => Some(MaterialType::Folder),
            "assignment" => Some(MaterialType::Assignment),
            "assessment" | "assessment_v2" | "test/quiz" | "quiz" => Some(MaterialType::Assessment),
            "document" | "gp" => Some(MaterialType::Document),
            "link" => Some(MaterialType::Link),
            _ => None,
        },
    }
}
