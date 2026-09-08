use crate::{
    filesystem::read_courses,
    types::{assignment::Assignment, course::Course, material::Material},
};

#[derive(Default)]
pub struct CourseState {
    courses: Vec<Course>,
}

impl CourseState {
    pub fn load_courses(&mut self) {
        match read_courses() {
            Ok(courses) => self.courses = courses,
            Err(_courses) => {} // todo!
        }
    }
    pub fn get_all_assignments(&self) -> impl Iterator<Item = &Assignment> {
        self.courses
            .iter()
            .flat_map(|c| c.materials.recursive_iter())
            .filter_map(|m| match m {
                Material::Assignment(m) => Some(m),
                _ => None,
            })
    }
}
