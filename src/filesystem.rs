use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Error},
    path::{Path, PathBuf},
};

use log::{error, warn};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{config::config, types::course::Course};

pub fn read_courses() -> io::Result<Vec<Course>> {
    let mut courses = vec![];
    for file_result in fs::read_dir(config().courses_dir())? {
        let path = file_result?.path();
        let reader = BufReader::new(File::open(&path)?);
        match serde_json::from_reader::<_, Course>(reader) {
            Ok(mut course) => {
                course.materials.set_course_id(&course.course_id);
                courses.push(course);
            }
            Err(error) => warn!("Failed to read course from {}: {error}", path.display()),
        }
    }
    if courses.is_empty() {
        warn!("courses is empty; reimporting");
        return Err(Error::other("course vec is empty"));
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
    let courses_dir = config().courses_dir();
    let course_json_path =
        courses_dir.join(sanitized_file(&courses_dir, &course.course_title, "json"));
    write_json(&course_json_path, course)?;
    Ok(course_json_path)
}

pub(super) fn read_json<T>(path: &Path) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let reader = BufReader::new(File::open(path)?);
    serde_json::from_reader(reader).map_err(Error::other)
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    create_if_missing(path)?;
    let writer = BufWriter::new(OpenOptions::new().write(true).truncate(true).open(path)?);
    serde_json::to_writer_pretty(writer, value)?;
    Ok(())
}

pub(super) fn create_if_missing(path: &Path) -> io::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

pub(super) fn sanitized_file(parent: &Path, title: &str, extension: &str) -> PathBuf {
    let stem = safe_stem(title);
    let name = format!("{stem}.{extension}");
    let path = parent.join(name);
    path
}

fn safe_stem(title: &str) -> String {
    let stem = title
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'..='\u{1f}' => '_',
            character => character,
        })
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_owned();
    if stem.is_empty() {
        "untitled".to_owned()
    } else {
        stem
    }
}
