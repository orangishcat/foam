pub mod courses;
mod folder;
mod material;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::types::material::Material;

pub use courses::write_courses;

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(path, format!("{json}\n"))
}

pub(super) fn material_title(material: &Material) -> &str {
    match material {
        Material::Folder(value) => &value.title,
        Material::Assessment(value) => &value.title,
        Material::Assignment(value) => &value.title,
        Material::Document(value) => &value.title,
        Material::Link(value) => &value.title,
    }
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
