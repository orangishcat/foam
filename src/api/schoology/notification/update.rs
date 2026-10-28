use crate::api::schoology::account::{ResetProgress, SchoologyAccountConfig};
use crate::ui;

const NOTIFICATION_PROGRESS_PRIORITY: u8 = 1;
const NOTIFICATION_PROGRESS_COLOR: slint::Color = slint::Color::from_rgb_u8(0x78, 0xaa, 0xff);
use crate::{
    api::schoology::RequestResult,
    database,
    thread_manager::check_cancelled,
    types::{
        assignment::Assignment,
        course::Course,
        notification::{Notification, NotificationEvent},
    },
};
use std::{collections::HashSet, io};

impl SchoologyAccountConfig {
    /// Process each course hierarchy once. Failed items remain pending.
    pub fn update_notifications(&self, notifications: &mut [Notification]) -> RequestResult<()> {
        let _reset = ResetProgress(NOTIFICATION_PROGRESS_PRIORITY, NOTIFICATION_PROGRESS_COLOR);
        let mut refreshed = HashSet::new();
        let total = notifications.len();
        let mut failure = None;
        let mut attempted = 0;
        ui::publish_progress(
            1.0,
            NOTIFICATION_PROGRESS_PRIORITY,
            &NOTIFICATION_PROGRESS_COLOR,
        );
        for i in 0..notifications.len() {
            // grades are batch processed
            if notifications[i].event == NotificationEvent::GradeUpdated {
                continue;
            }
            check_cancelled()?;
            let posted = notifications
                .iter()
                .filter(|n| {
                    n.event == NotificationEvent::MaterialPosted
                        && n.course_id == notifications[i].course_id
                })
                .map(|n| n.resource_id.clone())
                .collect();
            if let Err(err) =
                self.process_notification(&mut notifications[i], &posted, &mut refreshed)
            {
                log::warn!(
                    "Updating notification {} failed: {err}",
                    notifications[i].resource_id
                );
                failure = Some(err);
            }
            attempted += 1;
            ui::publish_progress(
                attempted as f32 / total as f32,
                NOTIFICATION_PROGRESS_PRIORITY,
                &NOTIFICATION_PROGRESS_COLOR,
            );
        }
        if notifications
            .iter()
            .any(|n| n.event == NotificationEvent::GradeUpdated)
        {
            check_cancelled()?;
            // One unfiltered request for this entire notification batch, including on failure.
            match self.fetch_grades().and_then(|grades| {
                self.apply_grades(&grades)?;
                Ok(())
            }) {
                Ok(()) => {
                    for n in notifications
                        .iter_mut()
                        .filter(|n| n.event == NotificationEvent::GradeUpdated)
                    {
                        let assignments = database::from_sql::<Assignment>(
                            "SELECT * FROM assignments WHERE id = ?".to_owned(),
                            &[&n.resource_id],
                        )?;
                        let Some(assignment) = assignments.into_iter().next() else {
                            continue;
                        };
                        if let Some(course) = crate::types::course::course(&n.course_id)? {
                            n.course_id = course.course_id;
                        } else {
                            n.course_id = assignment.course_id.clone();
                        }
                        n.subtitle = match (&assignment.score, &assignment.letter_grade) {
                            (Some(score), Some(letter)) => {
                                format!("{score} / {} ({letter})", assignment.max_points)
                            }
                            (Some(score), None) => format!("{score} / {}", assignment.max_points),
                            (None, Some(letter)) => letter.clone(),
                            (None, None) => String::new(),
                        };
                    }
                }
                Err(err) => {
                    log::warn!("Updating notification grades failed: {err}");
                    failure = Some(err);
                }
            }
            ui::publish_progress(
                0.0,
                NOTIFICATION_PROGRESS_PRIORITY,
                &NOTIFICATION_PROGRESS_COLOR,
            );
        }
        check_cancelled()?;
        match failure {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn resolve_notification_course(&self, n: &Notification) -> RequestResult<Course> {
        if n.course_id.is_empty() {
            return Err(io::Error::other("notification course ID is empty").into());
        }
        if let Some(course) = crate::types::course::course(&n.course_id)? {
            return Ok(course);
        }
        // fetch without holding the state lock; schoology section ids are sometimes incorrect for some reason
        let title = self.section_course_title(&n.course_id)?;
        Ok(crate::types::course::resolve_course_alias(
            &n.course_id,
            &title,
        )?)
    }

    fn process_notification(
        &self,
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
                let course = self.resolve_notification_course(n)?;
                if !refreshed.contains(&course.course_id) {
                    let cached = crate::types::material::materials(&course.course_id)?;
                    let materials = self.hierarchy(&course.course_id, &cached, posted)?;
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
                let kind = kinds.into_iter().next().ok_or_else(|| {
                    io::Error::other("posted material was not loaded from hierarchy")
                })?;
                n.material_type = Some(kind);
                n.course_id = course.course_id;
            }
            NotificationEvent::Unknown => {
                return Err(io::Error::other("unsupported notification event").into());
            }
        }
        Ok(())
    }
}
