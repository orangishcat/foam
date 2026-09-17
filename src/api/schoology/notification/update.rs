use crate::{
    api::schoology::{RequestResult, course},
    database,
    state::state::state,
    thread_manager::check_cancelled,
    types::{
        course::Course,
        material::Material,
        notification::{Notification, NotificationEvent},
    },
};
use std::{collections::HashSet, io};

/// Process each course hierarchy once. Failed items remain pending.
pub fn update(
    notifications: &mut [Notification],
    mut progress: impl FnMut(f32),
) -> RequestResult<()> {
    let mut refreshed = HashSet::new();
    let total = notifications.iter().filter(|n| !n.is_processed).count();
    let mut attempted = 0;
    for i in 0..notifications.len() {
        if notifications[i].is_processed
            || notifications[i].event == NotificationEvent::GradeUpdated
        {
            continue;
        }
        check_cancelled()?;
        let posted = notifications
            .iter()
            .filter(|n| {
                !n.is_processed
                    && n.event == NotificationEvent::MaterialPosted
                    && n.course_id == notifications[i].course_id
            })
            .map(|n| n.resource_id.clone())
            .collect();
        if let Err(err) = process(&mut notifications[i], &posted, &mut refreshed) {
            log::warn!(
                "Updating notification {} failed: {err}",
                notifications[i].resource_id
            );
        }
        attempted += 1;
        progress(attempted as f32 / total as f32);
    }
    if notifications
        .iter()
        .any(|n| !n.is_processed && n.event == NotificationEvent::GradeUpdated)
    {
        check_cancelled()?;
        // One unfiltered request for this entire notification batch, including on failure.
        match course::grades::fetch() {
            Ok(grades) => {
                let mut app = state();
                course::grades::apply(&mut app.course.courses, &grades);
                for n in notifications
                    .iter_mut()
                    .filter(|n| !n.is_processed && n.event == NotificationEvent::GradeUpdated)
                {
                    if let Some(a) = app
                        .course
                        .walk_assignments()
                        .find(|a| a.id == n.resource_id)
                    {
                        n.course_id = a.course_id.clone();
                    }
                    n.is_processed = true;
                }
            }
            Err(err) => log::warn!("Updating notification grades failed: {err}"),
        }
        progress(1.0);
    }
    check_cancelled()?;
    Ok(())
}

fn resolve_course(n: &Notification) -> RequestResult<Course> {
    if n.course_id.is_empty() {
        return Err(io::Error::other("notification course ID is empty").into());
    }
    if let Some(course) = database::from_sql(query, params).cloned() {
        return Ok(course);
    }
    // fetch without holding the state lock; schoology section ids are sometimes incorrect for some reason
    let title = course::courses::section_course_title(&n.course_id)?;
    let mut app = state();
    // Another worker may have resolved the alias while the request ran.
    if let Some(course) = app.course.get_course(&n.course_id) {
        return Ok(course.clone());
    }
    let mut matches = app
        .course
        .courses
        .iter_mut()
        .filter(|c| !title.is_empty() && c.course_title == title);
    let course = matches.next().ok_or_else(|| {
        io::Error::other(format!("no cached course matches section title {title:?}"))
    })?;
    if matches.next().is_some() {
        return Err(io::Error::other("ambiguous notification course title").into());
    }
    course.aliases.push(n.course_id.clone());
    let course = course.clone();
    Ok(course)
}

fn process(
    n: &mut Notification,
    posted: &HashSet<String>,
    refreshed: &mut HashSet<String>,
) -> RequestResult<()> {
    if n.resource_id.is_empty() {
        return Err(io::Error::other("notification resource ID is empty").into());
    }
    match n.event {
        NotificationEvent::GradeUpdated => unreachable!("grades are processed as a batch"),
        NotificationEvent::MaterialPosted => {
            let course = resolve_course(n)?;
            if !refreshed.contains(&course.course_id) {
                let mut materials =
                    course::hierarchy(&course.course_id, &course.materials, posted)?;
                materials.set_course_id(&course.course_id);
                check_cancelled()?;
                let mut app = state();
                let stored = app
                    .course
                    .courses
                    .iter_mut()
                    .find(|c| c.course_id == course.course_id)
                    .ok_or_else(|| io::Error::other("notification course was unloaded"))?;
                for material in materials.recursive_iter_mut() {
                    if let Material::Assignment(a) = material
                        && let Some(Material::Assignment(current)) = stored
                            .materials
                            .recursive_iter()
                            .find(|m| course::material_id(m) == a.id)
                    {
                        *a = current.clone();
                    }
                }
                stored.materials = materials;
                refreshed.insert(course.course_id.clone());
            }
            let app = state();
            let material = app
                .course
                .get_course(&course.course_id)
                .and_then(|c| {
                    c.materials
                        .recursive_iter()
                        .find(|m| course::material_id(m) == n.resource_id)
                })
                .ok_or_else(|| io::Error::other("posted material was not loaded from hierarchy"))?;
            n.material_type = Some(material.into());
            n.course_id = course.course_id;
        }
        NotificationEvent::Unknown => {
            return Err(io::Error::other("unsupported notification event").into());
        }
    }
    n.is_processed = true;
    Ok(())
}
