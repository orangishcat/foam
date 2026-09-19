use std::{
    cmp::min,
    io::{self, Error, Result},
};

use chrono::{DateTime, Local, TimeDelta};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, UiState,
    api::{self, schoology},
    config::config,
    database,
    state::{courses, state::state},
    thread_manager::{self, check_cancelled},
    types::{
        assignment::Assignment,
        material::MaterialType,
        notification::{self, Notification},
        submission,
    },
    ui,
};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct NotificationState {
    pub last_update: DateTime<Local>,
    pub last_sync: DateTime<Local>,
    pub last_submission_sync: DateTime<Local>,
    #[serde(skip)]
    last_update_success: bool,
    #[serde(skip)]
    scrape_attempts: u32,
    #[serde(skip)]
    is_checking_notifications: bool,
    #[serde(skip)]
    is_checking_submissions: bool,
}

impl NotificationState {
    pub fn load(&mut self) {
        match load_sync_state() {
            Ok(saved) => *self = saved,
            Err(err) => log::warn!("Loading notification sync state failed: {err}"),
        }
    }

    pub fn check_notifications(&mut self) {
        if !self.is_checking_notifications
            && Local::now() - self.last_update >= self.refresh_duration()
        {
            self.is_checking_notifications = true;
            self.scrape_attempts += 1;
            if let Err(err) =
                thread_manager::spawn_thread("sync notifications", Self::sync_notifications)
            {
                self.is_checking_notifications = false;
                log::warn!("Starting notification sync failed: {err}");
            }
        }
        // Submission sync can run alongside notifications: each updates only its own SQL columns.
        if !self.is_checking_submissions
            && Local::now() - self.last_submission_sync >= config().submission_refresh_duration
        {
            self.is_checking_submissions = true;
            if let Err(err) =
                thread_manager::spawn_thread("sync submissions", Self::sync_submissions)
            {
                self.is_checking_submissions = false;
                log::warn!("Starting submission sync failed: {err}");
            }
        }
    }

    fn refresh_duration(&self) -> TimeDelta {
        if self.last_update_success {
            config().refresh_duration
        } else {
            min(
                api::exponential_retry(self.scrape_attempts),
                config().refresh_duration,
            )
        }
    }

    fn sync_notifications() {
        let check_started = Local::now();
        let result = (|| -> schoology::RequestResult<()> {
            courses::ensure_loaded()?;
            let mut notifications = schoology::notification::scrape_notifications()?;
            let previous = notification::notifications()?;
            let last_sync = state().notif.last_sync;
            for n in &mut notifications {
                n.is_processed = previous
                    .iter()
                    .find(|old| {
                        old.event == n.event
                            && old.resource_id == n.resource_id
                            && old.created == n.created
                    })
                    .map_or(n.created < last_sync, |old| old.is_processed);
            }
            check_cancelled()?;
            let update_result =
                schoology::notification::update::update(&mut notifications, Self::publish_progress);
            Self::sync_calendar();
            let mut app = state();
            let mut next = app.notif.clone();
            if update_result.is_ok() && notifications.iter().all(|n| n.is_processed) {
                next.last_sync = check_started;
            }
            next.last_update = Local::now();
            next.last_update_success =
                update_result.is_ok() && notifications.iter().all(|n| n.is_processed);
            if next.last_update_success {
                next.scrape_attempts = 0;
            }
            notification::save_notifications(&notifications, &next)?;
            app.notif = next;
            update_result
        })();
        {
            let mut app = state();
            app.notif.is_checking_notifications = false;
            if let Err(err) = result {
                app.notif.last_update_success = false;
                app.notif.last_update = Local::now();
                log::warn!("Notification sync failed: {err}");
            }
        }
        ui::sync_ui();
    }

    fn sync_calendar() {
        match schoology::calendar::fetch()
            .and_then(|(ids, assignments)| Ok(schoology::calendar::apply(&ids, &assignments)?))
        {
            Ok(updated) => log::debug!("Updated {updated} calendar assignments"),
            Err(err) => log::warn!("Calendar sync failed: {err}"),
        }
    }

    fn sync_submissions() {
        let check_started = Local::now();
        let result = (|| -> schoology::RequestResult<()> {
            // Initial course loading fetches submissions itself; do not mark an empty cache synced.
            let loaded = database::from_sql_map(
                "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
                &[],
                |row| row.get::<_, String>(0).map_err(Error::other),
            )?;
            if loaded.is_empty() {
                return Ok(());
            }
            let mut assignments = database::from_sql::<Assignment>(
                format!(
                    "SELECT * FROM assignments WHERE julianday(due) < julianday('now') AND NOT ({})",
                    include_str!("../sql/assignment/completed.sql")
                ),
                &[],
            )?;
            schoology::course::submissions::scrape_submissions(&mut assignments)?;
            check_cancelled()?;
            submission::update_submissions(&assignments)?;
            let mut app = state();
            let mut next = app.notif.clone();
            next.last_submission_sync = check_started;
            save_sync_state(&next)?;
            app.notif = next;
            Ok(())
        })();
        state().notif.is_checking_submissions = false;
        if let Err(err) = result {
            log::warn!("Submission sync failed: {err}");
        }
        ui::sync_ui();
    }

    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        // Persisted canonical course IDs are preferred; the feed title covers unresolved IDs.
        let notifications = database::from_sql_map(
            "SELECT n.*, COALESCE(c.course_title, NULLIF(n.course_title, ''), 'Unknown course') AS display_course
             FROM notifications n LEFT JOIN courses c ON n.course_id = c.course_id
             ORDER BY julianday(n.created) DESC, n.id".to_owned(), &[], |row| {
                Ok((serde_rusqlite::from_row::<Notification>(row).map_err(Error::other)?,
                    row.get::<_, String>("display_course").map_err(Error::other)?))
            })?;
        let models = notifications
            .into_iter()
            .map(|(n, course)| crate::Notification {
                title: n.title.into(),
                course: course.into(),
                course_id: n.course_id.into(),
                n_type: match n.material_type {
                    Some(MaterialType::Assignment | MaterialType::Assessment) => {
                        crate::NotificationType::NewAssignment
                    }
                    Some(MaterialType::Document) => crate::NotificationType::NewDocument,
                    Some(MaterialType::Link) => crate::NotificationType::NewLink,
                    _ => crate::NotificationType::Unknown,
                },
                icon_color: match n.material_type {
                    Some(MaterialType::Assignment | MaterialType::Assessment) => {
                        ui.global::<UiState>().get_theme().accent_400
                    }
                    _ => ui.global::<UiState>().get_theme().text_400,
                },
            })
            .collect::<Vec<_>>();
        let global = ui.global::<crate::NotificationUi>();
        global.set_progress(0.0);
        global.set_temp_notifs(ModelRc::new(VecModel::from(Vec::new())));
        global.set_notifications(ModelRc::new(VecModel::from(models)));
        Ok(())
    }

    fn publish_progress(progress: f32) {
        ui::run_on_ui_thread(move |ui| {
            ui.global::<crate::NotificationUi>().set_progress(progress);
        });
    }
}

pub fn load_sync_state<T: DeserializeOwned + Default>() -> Result<T> {
    let values = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = 'notifications'".to_owned(),
        &[],
        |row| {
            let data: String = row.get(0).map_err(Error::other)?;
            serde_json::from_str(&data).map_err(Error::other)
        },
    )?;
    Ok(values.into_iter().next().unwrap_or_default())
}

pub fn save_sync_state(state: &impl Serialize) -> Result<()> {
    let connection = database::connection()?;
    save_sync_state_on(&connection, state)
}

pub(crate) fn save_sync_state_on(connection: &Connection, state: &impl Serialize) -> Result<()> {
    connection.execute("INSERT INTO sync_state (key, data) VALUES ('notifications', ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data",
        params![serde_json::to_string(state).map_err(Error::other)?]).map_err(Error::other)?;
    Ok(())
}
