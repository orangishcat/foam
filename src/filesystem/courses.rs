use std::{
    fs::{self, File},
    io::{self, BufReader},
    path::PathBuf,
};

use log::error;

use crate::{config::config, filesystem::safe_stem, types::course::Course};

use super::write_json;

pub fn read_courses() -> io::Result<Vec<Course>> {
    let mut courses = vec![];
    for file_result in fs::read_dir(config().courses_dir())? {
        let path = file_result?.path();
        let reader = BufReader::new(File::open(&path)?);
        match serde_json::from_reader(reader) {
            Ok(course) => courses.push(course),
            Err(error) => error!("Failed to read course from {}: {error}", path.display()),
        }
    }
    Ok(courses)
}

pub fn write_courses(courses: &[Course]) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(config().courses_dir())?;
    Ok(courses
        .iter()
        .filter_map(|course| match write_course(course) {
            Ok(path) => Some(path),
            Err(error) => {
                error!("Failed to write course: {error}");
                None
            }
        })
        .collect())
}

pub fn write_course(course: &Course) -> io::Result<PathBuf> {
    let course_json_path = config()
        .courses_dir()
        .join(safe_stem(&course.course_title) + ".json");
    write_json(&course_json_path, course)?;
    Ok(course_json_path)
}
