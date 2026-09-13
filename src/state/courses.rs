use std::{error::Error, iter};

use itertools::Itertools;
use log::error;

use crate::{
    api::schoology,
    filesystem::{self, read_courses},
    types::{course::Course, material::Material},
};

#[derive(Clone, Default)]
pub struct CourseState {
    pub courses: Vec<Course>,
}

impl CourseState {
    pub fn load(&mut self) {
        match read_courses() {
            Ok(courses) => {
                self.courses = courses;
                log::info!(
                    "Loaded {} courses:\n{}",
                    self.courses.len(),
                    self.courses
                        .iter()
                        .map(|c| format!(
                            "\t{} ({})",
                            c.course_title,
                            [vec![c.course_id.clone()], c.aliases.clone()]
                                .concat()
                                .join(", ")
                        ))
                        .sorted()
                        .collect::<Vec<String>>()
                        .join("\n")
                )
            }
            Err(_err) => {
                if let Ok(courses) = self
                    .scrape_and_write_courses()
                    .inspect_err(|e| error!("An error occured while scraping courses: {}", e))
                {
                    self.courses = courses;
                };
            }
        }
    }
    pub fn get_course(&self, course_id: &str) -> Option<&Course> {
        self.courses
            .iter()
            .find(|course| course.course_id == course_id)
            .or_else(|| {
                self.courses
                    .iter()
                    .find(|course| course.aliases.iter().any(|alias| alias == course_id))
            })
    }
    pub fn walk_materials(&self) -> impl Iterator<Item = &Material> + '_ {
        self.courses
            .iter()
            .flat_map(|c| c.materials.recursive_iter())
    }
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        filesystem::write_courses(&self.courses)?;
        Ok(())
    }
    pub fn scrape_and_write_courses(&self) -> Result<Vec<Course>, Box<dyn Error + Send + Sync>> {
        let courses = schoology::course::courses::scrape_courses()?;
        filesystem::write_courses(&courses)?;
        Ok(courses)
    }
}
