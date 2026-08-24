use crate::types::course::Course;

pub struct AppState {
    courses: Vec<Course>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            courses: Default::default(),
        }
    }
}
