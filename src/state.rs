use crate::types::course::Course;

#[derive(Default)]
pub struct AppState {
    courses: Vec<Course>,
}
