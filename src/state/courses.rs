use std::collections::BTreeMap;

use crate::{
    filesystem::read_courses,
    types::{course::Course, material::Material},
};

#[derive(Default)]
pub struct CourseState {
    courses: BTreeMap<String, Course>,
}

impl CourseState {
    pub fn load_courses(&mut self) {
        match read_courses() {
            Ok(courses) => {
                self.courses = courses
                    .into_iter()
                    .map(|course| (course.course_id.clone(), course))
                    .collect();
            }
            Err(_courses) => {} // todo!
        }
    }
    pub fn get_course(&self, course_id: &str) -> Option<&Course> {
        self.courses.get(course_id)
    }

    pub fn walk_materials(&self) -> impl Iterator<Item = &Material> + '_ {
        self.courses
            .values()
            .flat_map(|c| c.materials.recursive_iter())
    }
}
