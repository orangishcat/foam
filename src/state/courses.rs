use std::{collections::BTreeMap, error::Error};

use log::error;

use crate::{
    filesystem::{self, read_courses},
    schoology::{self, course::submissions::scrape_submissions},
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
                self.courses = Self::to_btree(courses);
            }
            Err(_err) => {
                fn scrape_courses() -> Result<Vec<Course>, Box<dyn Error + Send + Sync>> {
                    let courses = schoology::course::courses::scrape_courses()?;
                    filesystem::write_courses(&courses)?;
                    Ok(courses)
                }
                if let Ok(courses) = scrape_courses()
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
    fn to_btree(courses: Vec<Course>) -> BTreeMap<String, Course> {
        courses
            .into_iter()
            .map(|course| (course.course_id.clone(), course))
            .collect()
    }
}
