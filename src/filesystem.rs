use std::{
    fs::{self, File},
    io::{self, BufReader},
    path::{Path, PathBuf},
};

use log::error;
use serde::Serialize;

use crate::{
    config::config,
    types::{course::Course, material::Material},
};

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
    let courses_dir = config().courses_dir();
    let course_json_path =
        courses_dir.join(unique_file(&courses_dir, &course.course_title, ".json"));
    write_json(&course_json_path, course)?;
    Ok(course_json_path)
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(path, format!("{json}\n"))
}

pub(super) fn unique_directory(parent: &Path, title: &str) -> io::Result<PathBuf> {
    let stem = safe_stem(title);
    for duplicate in 1usize.. {
        let name = if duplicate == 1 {
            stem.clone()
        } else {
            format!("{stem} ({duplicate})")
        };
        let path = parent.join(name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

pub(super) fn unique_file(parent: &Path, title: &str, extension: &str) -> PathBuf {
    let stem = safe_stem(title);
    for duplicate in 1usize.. {
        let name = if duplicate == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({duplicate}).{extension}")
        };
        let path = parent.join(name);
        if !path.exists() {
            return path;
        }
    }
    unreachable!()
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
