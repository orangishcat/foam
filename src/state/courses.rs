use std::{collections::BTreeMap, error::Error};

use log::error;

use crate::{
    api::schoology,
    filesystem::{self, read_courses},
    types::{course::Course, material::Material},
};

#[derive(Clone, Default)]
pub struct CourseState {
    pub courses: BTreeMap<String, Course>,
}

impl CourseState {
    pub fn load_courses(&mut self) {
        match read_courses() {
            Ok(courses) => {
                self.courses = Self::to_btree(courses);
            }
            Err(_err) => {
                if let Ok(courses) = self
                    .scrape_and_write_courses()
                    .inspect_err(|e| error!("An error occured while scraping courses: {}", e))
                {
                    self.courses = Self::to_btree(courses);
                };
            }
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
    pub fn save(&self) {
        let courses: Vec<Course> = self.courses.values().cloned().collect();
        filesystem::write_courses(&courses);
    }
    pub fn scrape_and_write_courses(&self) -> Result<Vec<Course>, Box<dyn Error + Send + Sync>> {
        let courses = schoology::course::courses::scrape_courses()?;
        filesystem::write_courses(&courses)?;
        Ok(courses)
    }
    fn to_btree(courses: Vec<Course>) -> BTreeMap<String, Course> {
        courses
            .into_iter()
            .map(|course| (course.course_id.clone(), course))
            .collect()
    }
}
