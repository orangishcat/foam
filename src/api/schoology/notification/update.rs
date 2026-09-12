use std::{
    collections::{HashMap, HashSet},
    io,
};

use scraper::{Html, Selector};

use crate::{
    api::schoology::{
        self, RequestResult,
        course::{self, CourseMaterial, materials},
    },
    types::{
        course::Course,
        folder::Folder,
        material::{Material, MaterialType},
        notification::{Notification, NotificationEvent},
    },
};

// Paths remain valid while appending materials. Rebuild after replacing a hierarchy.
type FolderMap = HashMap<(String, String), Vec<usize>>;

fn index_folder(course: &str, folder: &Folder, path: &mut Vec<usize>, index: &mut FolderMap) {
    index.insert((course.to_owned(), folder.id.clone()), path.clone());
    for (i, material) in folder.materials.iter().enumerate() {
        if let Material::Folder(child) = material {
            path.push(i);
            index_folder(course, child, path, index);
            path.pop();
        }
    }
}

fn folder_at<'a>(mut folder: &'a mut Folder, path: &[usize]) -> &'a mut Folder {
    for &i in path {
        let Material::Folder(child) = &mut folder.materials[i] else {
            unreachable!("indexed folder path")
        };
        folder = child;
    }
    folder
}

fn assignment_parent(html: &str, course_id: &str) -> RequestResult<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse(
        ".content-top-upper-wrapper > div:nth-child(2) > div:nth-child(2) > a:nth-child(2)",
    )
    .unwrap();
    let href = document
        .select(&selector)
        .next()
        .and_then(|a| a.value().attr("href"))
        .ok_or_else(|| io::Error::other("assignment parent folder anchor is missing"))?;
    let url = reqwest::Url::parse("https://schoology.com")?.join(href)?;
    if url.path() != format!("/course/{course_id}/materials") {
        return Err(io::Error::other("assignment breadcrumb belongs to another course").into());
    }
    Ok(url
        .query_pairs()
        .find(|(key, _)| key == "f")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "0".to_owned()))
}

/// Discover parents first, then handle pending notifications in input order.
/// A failed notification remains pending; progress only advances after success.
pub fn update(
    notifications: &mut [Notification],
    courses: &mut [Course],
    mut publish_progress: impl FnMut(f32),
) -> RequestResult<()> {
    let mut folders = FolderMap::new();
    let course_indices: HashMap<_, _> = courses
        .iter()
        .enumerate()
        .map(|(i, c)| (c.course_id.clone(), i))
        .collect();
    for course in courses.iter() {
        index_folder(
            &course.course_id,
            &course.materials,
            &mut Vec::new(),
            &mut folders,
        );
        folders.insert((course.course_id.clone(), "0".into()), Vec::new());
    }
    let pending: Vec<_> = notifications
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.is_processed)
        .map(|(i, _)| i)
        .collect();
    let mut parents = HashMap::new();
    let mut documents = HashMap::new();
    for &i in &pending {
        let n = &notifications[i];
        if !course_indices.contains_key(&n.course_id) {
            return Err(io::Error::other(format!(
                "notification course {} is not loaded",
                n.course_id
            ))
            .into());
        }
        if n.event != NotificationEvent::MaterialPosted {
            continue;
        }
        let parent = match n.material_type {
            Some(MaterialType::Document) => {
                let document: materials::document::Document = schoology::api_get(&format!(
                    "https://api.schoology.com/v1/sections/{}/documents/{}",
                    n.course_id, n.resource_id
                ))?;
                let parent = document.course_fid.0.to_string();
                documents.insert(i, Material::Document(document.into()));
                parent
            }
            Some(MaterialType::Assignment | MaterialType::Assessment) => assignment_parent(
                &schoology::internal_get_html(&format!("/assignment/{}", n.resource_id))?,
                &n.course_id,
            )?,
            Some(MaterialType::Link) => {
                let meta = descriptor(n, "link", "documents");
                let link = materials::link::scrape(&meta, meta.location.as_deref().unwrap())?;
                let parent = link.course_fid.to_string();
                documents.insert(i, Material::Link(link));
                parent
            }
            _ => continue,
        };
        parents.insert(i, parent);
    }
    let mut refreshed = HashSet::new();
    for (done, &i) in pending.iter().enumerate() {
        let n = &mut notifications[i];
        let course = &mut courses[course_indices[&n.course_id]];
        match n.event {
            NotificationEvent::MaterialPosted => {
                let missing = parents.get(&i).is_some_and(|parent| {
                    !folders.contains_key(&(n.course_id.clone(), parent.clone()))
                });
                if (missing || n.material_type == Some(MaterialType::Folder))
                    && refreshed.insert(n.course_id.clone())
                {
                    course.materials = course::hierarchy(&n.course_id, &course.materials)?;
                    folders.retain(|(id, _), _| id != &n.course_id);
                    index_folder(
                        &n.course_id,
                        &course.materials,
                        &mut Vec::new(),
                        &mut folders,
                    );
                    folders.insert((n.course_id.clone(), "0".into()), Vec::new());
                }
                if let Some(parent) = parents.get(&i) {
                    let path = folders
                        .get(&(n.course_id.clone(), parent.clone()))
                        .ok_or_else(|| {
                            io::Error::other(format!(
                                "parent folder {parent} is absent after hierarchy refresh"
                            ))
                        })?;
                    let material = if let Some(material) = documents.remove(&i) {
                        material
                    } else {
                        let kind = if n.material_type == Some(MaterialType::Assessment) {
                            "assessment"
                        } else {
                            "assignment"
                        };
                        let meta = descriptor(n, kind, "assignments");
                        let mut material = materials::scrape(&meta)?
                            .ok_or_else(|| io::Error::other("unsupported posted material"))?;
                        if let Material::Assignment(a) = &mut material {
                            a.course_id = n.course_id.clone();
                        }
                        material
                    };
                    let folder = folder_at(&mut course.materials, path);
                    // Repeated feed entries must not duplicate or erase cached grades/submissions.
                    if !folder.materials.iter().any(|m| {
                        course::material_id(m) == n.resource_id
                            && MaterialType::from(m) == MaterialType::from(&material)
                    }) {
                        folder.materials.push(material);
                    }
                }
            }
            NotificationEvent::GradeUpdated => {
                course::grades::scrape_grades(std::slice::from_mut(course))?;
            }
            NotificationEvent::Unknown => {}
        }
        n.is_processed = true;
        publish_progress((done + 1) as f32 / pending.len() as f32);
    }
    Ok(())
}

fn descriptor(n: &Notification, kind: &str, endpoint: &str) -> CourseMaterial {
    CourseMaterial {
        id: n.resource_id.clone(),
        title: n.title.clone(),
        body: String::new(),
        material_type: kind.into(),
        location: Some(format!(
            "https://api.schoology.com/v1/sections/{}/{endpoint}/{}",
            n.course_id, n.resource_id
        )),
    }
}
