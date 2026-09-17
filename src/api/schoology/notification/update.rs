use crate::{
    api::schoology::{RequestResult, course},
    database,
    thread_manager::check_cancelled,
    types::{
        assignment::Assignment,
        course::Course,
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
        match course::grades::fetch().and_then(|grades| {
            course::grades::apply(&grades)?;
            Ok(())
        }) {
            Ok(()) => {
                for n in notifications
                    .iter_mut()
                    .filter(|n| !n.is_processed && n.event == NotificationEvent::GradeUpdated)
                {
                    if let Some(course) = crate::types::course::course(&n.course_id)? {
                        n.course_id = course.course_id;
                    } else {
                        let assignments = database::from_sql::<Assignment>(
                            "SELECT * FROM assignments WHERE id = ?".to_owned(),
                            &[&n.resource_id],
                        )?;
                        if assignments.len() == 1 {
                            n.course_id = assignments[0].course_id.clone();
                        }
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
    if let Some(course) = crate::types::course::course(&n.course_id)? {
        return Ok(course);
    }
    // fetch without holding the state lock; schoology section ids are sometimes incorrect for some reason
    let title = course::courses::section_course_title(&n.course_id)?;
    Ok(crate::types::course::resolve_course_alias(
        &n.course_id,
        &title,
    )?)
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
                let cached = crate::types::material::materials(&course.course_id)?;
                let materials = course::hierarchy(&course.course_id, &cached, posted)?;
                check_cancelled()?;
                crate::types::material::store_hierarchy(&course.course_id, &materials)?;
                refreshed.insert(course.course_id.clone());
            }
            let kinds = database::from_sql_map(
                "SELECT type FROM materials WHERE course_id = ? AND material_id = ?".to_owned(),
                &[&course.course_id, &n.resource_id],
                |row| {
                    let kind: String = row.get(0).map_err(io::Error::other)?;
                    serde_json::from_value(serde_json::Value::String(kind))
                        .map_err(io::Error::other)
                },
            )?;
            let kind = kinds
                .into_iter()
                .next()
                .ok_or_else(|| io::Error::other("posted material was not loaded from hierarchy"))?;
            n.material_type = Some(kind);
            n.course_id = course.course_id;
        }
        NotificationEvent::Unknown => {
            return Err(io::Error::other("unsupported notification event").into());
        }
    }
    n.is_processed = true;
    Ok(())
}
