use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::types::course::Course;

use super::{folder::write_folder_contents, unique_directory, write_json};

pub fn write_courses(courses: &[Course], destination: &Path) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(destination)?;
    courses
        .iter()
        .map(|course| write_course(course, destination))
        .collect()
}

pub fn write_course(course: &Course, destination: &Path) -> io::Result<PathBuf> {
    let directory = unique_directory(
        destination,
        &format!("{}__{}", course.course_title, course.section_title),
    )?;
    write_json(&directory.join("course.json"), course)?;
    write_folder_contents(&course.materials, &directory.join("materials"))?;
    Ok(directory)
}
