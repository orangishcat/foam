use crate::{filesystem::read_courses, types::course::Course};

#[derive(Default)]
pub struct AppState {
    courses: Vec<Course>,
}

impl AppState {
    pub fn load_courses(&mut self) {
        match read_courses() {
            Ok(courses) => self.courses = courses,
            Err(_courses) => {}
        }
    }
}
