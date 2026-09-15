use std::{
    collections::HashMap,
    io::{self},
};

use scraper::{Html, Selector};

use crate::{
    api::schoology::{
        self, RequestResult,
        course::{self, CourseMaterial, materials},
    },
    state::state::state,
    thread_manager::check_cancelled,
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

fn assignment_parent(html: &str, course: &Course) -> RequestResult<String> {
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
    if !std::iter::once(&course.course_id)
        .chain(course.aliases.iter())
        .any(|id| url.path() == format!("/course/{id}/materials"))
    {
        return Err(io::Error::other("assignment breadcrumb belongs to another course").into());
    }
    Ok(url
        .query_pairs()
        .find(|(key, _)| key == "f")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "0".to_owned()))
}

/// Discover each parent and process its notification together in input order.
/// Failed notifications are logged and remain pending; progress tracks attempted items.
pub fn update(
    notifications: &mut [Notification],
    mut publish_progress: impl FnMut(f32),
) -> RequestResult<()> {
    // 1. list pending notifications
    let mut pending: Vec<&mut Notification> = notifications
        .iter_mut()
        .filter(|n| !n.is_processed)
        .collect();
    if pending.is_empty() {
        return Ok(());
    }

    // 2. build folder index so that looking for folders in future calls is faster
    let mut folders = FolderMap::new();
    for course in &state().course.courses {
        index_folder(
            &course.course_id,
            &course.materials,
            &mut Vec::new(),
            &mut folders,
        );
        folders.insert((course.course_id.clone(), "0".into()), Vec::new());
    }

    // 3. process notifications
    let len = pending.len();
    for (i, notif) in pending.iter_mut().enumerate() {
        if let Err(err) = process_notification(notif, &mut folders) {
            log::warn!(
                "Skipping notification for resource {} in course {}: {err}",
                notif.title,
                notif.course_id
            );
        }
        publish_progress((i + 1) as f32 / len as f32);
        check_cancelled()?;
    }
    log::info!("Finished processing notifications");
    Ok(())
}

fn process_notification(n: &mut Notification, folders: &mut FolderMap) -> RequestResult<()> {
    crate::thread_manager::check_cancelled();
    if n.resource_id.is_empty() {
        return Err(
            io::Error::other(format!("Resource id for course {} is empty", n.course_id)).into(),
        );
    }
    if n.course_id.is_empty() {
        // probably grade posted (doesn't show course in schoology)
        let course_id_option = state()
            .course
            .walk_assignments()
            .filter_map(|a| {
                if a.id == n.resource_id {
                    Some(a.course_id.to_owned())
                } else {
                    None
                }
            })
            .next();

        if let Some(course_id) = course_id_option {
            n.course_id = course_id;
        } else {
            return Err(io::Error::other(format!(
                "Course id for resource {} is empty",
                n.resource_id
            ))
            .into());
        }
    }

    // Release the state lock before making requests or publishing progress.
    let course = state().course.get_course(&n.course_id).cloned();

    let mut course = match course {
        Some(course) => course,
        None => {
            let title = course::courses::section_course_title(&n.course_id)?;
            let mut state = state();
            // Another update may have resolved this ID while the request ran.
            if let Some(course) = state.course.get_course(&n.course_id) {
                course.clone()
            } else {
                let mut matches = state
                    .course
                    .courses
                    .iter_mut()
                    .filter(|course| !title.is_empty() && course.course_title == title);
                let course = matches.next().ok_or_else(|| {
                    io::Error::other(format!(
                        "notification course {} is not loaded: no course named {title:?}",
                        n.course_id
                    ))
                })?;
                if matches.next().is_some() {
                    return Err(io::Error::other(format!(
                        "notification course {} matches multiple courses named {title:?}",
                        n.course_id
                    ))
                    .into());
                }
                course.aliases.push(n.course_id.clone());
                course.clone()
            }
        }
    };
    // Commit refreshed paths with the course so a failure cannot leave stale paths.
    let mut updated_folders = folders.clone();
    match n.event {
        NotificationEvent::MaterialPosted => {
            handle_material_posted(n, &mut course, &mut updated_folders)?;
        }
        NotificationEvent::GradeUpdated => {
            course::grades::scrape_grades(std::slice::from_mut(&mut course))?;
        }
        NotificationEvent::Unknown => {}
    }
    let mut state = state();
    let stored = state
        .course
        .courses
        .iter_mut()
        .find(|stored| stored.course_id == course.course_id)
        .ok_or_else(|| io::Error::other("notification course was unloaded during update"))?;
    for alias in &stored.aliases {
        if !course.aliases.contains(alias) {
            course.aliases.push(alias.clone());
        }
    }
    *stored = course;
    *folders = updated_folders;
    n.is_processed = true;
    Ok(())
}

fn handle_material_posted(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
) -> RequestResult<()> {
    match n.material_type {
        Some(MaterialType::Document) => handle_document_posted(n, course, folders),
        Some(MaterialType::Assignment | MaterialType::Assessment) => {
            handle_assignment_posted(n, course, folders)
        }
        Some(MaterialType::Link) => handle_link_posted(n, course, folders),
        Some(MaterialType::Folder) => handle_folder_posted(n, course, folders),
        None => Ok(()),
    }
}

fn handle_document_posted(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
) -> RequestResult<()> {
    if contains_material(n, course) {
        return Ok(());
    }
    let document: materials::document::Document = schoology::api_get(&format!(
        "https://api.schoology.com/v1/sections/{}/documents/{}",
        course.course_id, n.resource_id
    ))?;
    let parent = document.course_fid.0.to_string();
    if let Some(path) = resolve_parent(n, course, folders, &parent)? {
        folder_at(&mut course.materials, &path)
            .materials
            .push(Material::Document(document.into()));
    }
    Ok(())
}

fn handle_assignment_posted(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
) -> RequestResult<()> {
    if contains_material(n, course) {
        return Ok(());
    }
    let (_, parent) = assignment_location(&n.resource_id, std::slice::from_ref(course))?;
    let Some(path) = resolve_parent(n, course, folders, &parent)? else {
        return Ok(());
    };
    let kind = if n.material_type == Some(MaterialType::Assessment) {
        "assessment"
    } else {
        "assignment"
    };
    let meta = descriptor(n, &course.course_id, kind, "assignments");
    let mut material =
        materials::scrape(&meta)?.ok_or_else(|| io::Error::other("unsupported posted material"))?;
    if let Material::Assignment(a) = &mut material {
        a.course_id = course.course_id.clone();
    }
    folder_at(&mut course.materials, &path)
        .materials
        .push(material);
    Ok(())
}

fn handle_link_posted(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
) -> RequestResult<()> {
    if contains_material(n, course) {
        return Ok(());
    }
    let meta = descriptor(n, &course.course_id, "link", "documents");
    let link = materials::link::scrape(&meta, meta.location.as_deref().unwrap())?;
    let parent = link.course_fid.to_string();
    if let Some(path) = resolve_parent(n, course, folders, &parent)? {
        folder_at(&mut course.materials, &path)
            .materials
            .push(Material::Link(link));
    }
    Ok(())
}

fn handle_folder_posted(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
) -> RequestResult<()> {
    if contains_material(n, course) {
        return Ok(());
    }
    refresh_hierarchy(course, folders)?;
    if !contains_material(n, course) {
        return Err(io::Error::other(format!(
            "folder {} is absent after hierarchy refresh",
            n.resource_id
        ))
        .into());
    }
    Ok(())
}

fn contains_material(n: &Notification, course: &Course) -> bool {
    // Repeated feed entries must not duplicate or erase cached grades/submissions.
    course.materials.recursive_iter().any(|material| {
        course::material_id(material) == n.resource_id
            && Some(MaterialType::from(material)) == n.material_type
    })
}

/// Returns no path if refreshing the hierarchy also loaded the material.
fn resolve_parent(
    n: &Notification,
    course: &mut Course,
    folders: &mut FolderMap,
    parent: &str,
) -> RequestResult<Option<Vec<usize>>> {
    let key = (course.course_id.clone(), parent.to_owned());
    if !folders.contains_key(&key) {
        refresh_hierarchy(course, folders)?;
        if contains_material(n, course) {
            return Ok(None);
        }
    }
    folders.get(&key).cloned().map(Some).ok_or_else(|| {
        io::Error::other(format!(
            "parent folder {parent} is absent after hierarchy refresh"
        ))
        .into()
    })
}

fn refresh_hierarchy(course: &mut Course, folders: &mut FolderMap) -> RequestResult<()> {
    course.materials = course::hierarchy(&course.course_id, &course.materials)?;
    folders.retain(|(id, _), _| id != &course.course_id);
    index_folder(
        &course.course_id,
        &course.materials,
        &mut Vec::new(),
        folders,
    );
    folders.insert((course.course_id.clone(), "0".into()), Vec::new());
    Ok(())
}

fn descriptor(n: &Notification, course_id: &str, kind: &str, endpoint: &str) -> CourseMaterial {
    CourseMaterial {
        id: n.resource_id.clone(),
        title: n.title.clone(),
        body: String::new(),
        material_type: kind.into(),
        location: Some(format!(
            "https://api.schoology.com/v1/sections/{}/{endpoint}/{}",
            course_id, n.resource_id
        )),
    }
}
