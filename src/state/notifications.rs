use std::{collections::HashMap, path::PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, UiState,
    api::schoology,
    config::config,
    filesystem,
    state::{courses::CourseState, state::state},
    thread_manager::{self, check_cancelled},
    types::{
        material::{Material, MaterialType},
        notification::Notification,
    },
    ui,
};

const NOTIFICATION_FILE: &str = "notifications.json";

#[derive(Default, Serialize, Deserialize)]
pub struct NotificationState {
    pub notifications: Vec<Notification>,

    pub last_update: DateTime<Local>,
    pub last_sync: DateTime<Local>,
    pub last_submission_sync: DateTime<Local>,

    #[serde(skip)]
    is_checking_notifications: bool,
    #[serde(skip)]
    is_checking_submissions: bool,
}

impl NotificationState {
    pub fn load(&mut self) {
        *self = filesystem::read_json(&Self::notification_path())
            .inspect_err(|e| log::warn!("Failed to read notifications: {e}"))
            .unwrap_or_default();
    }
    pub fn save(&self) {
        filesystem::write_json(&Self::notification_path(), &self)
            .inspect_err(|e| log::warn!("Failed to write notifications: {e}"))
            .unwrap_or_default();
    }
    pub fn notification_path() -> PathBuf {
        config().data_dir().join(NOTIFICATION_FILE)
    }
    pub fn check_notifications(&mut self) {
        if Local::now() - self.last_update < config().refresh_duration
            || self.is_checking_notifications
        {
            log::debug!("Notifications cache is fresh, skipping fetch");
            return;
        }

        self.is_checking_notifications = true;

        if let Err(err) = Self::spawn_notif_scrape() {
            self.is_checking_notifications = false;
            log::warn!("Scraping notifications failed: {err}");
        }

        if Local::now() - self.last_submission_sync < config().submission_refresh_duration
            || self.is_checking_notifications
        {
            log::debug!("Submissions cache is fresh, skipping fetch");
            return;
        }

        self.is_checking_submissions = true;
        if let Err(err) = Self::spawn_submission_sync() {
            self.is_checking_submissions = false;
            log::warn!("Updating submissions failed: {err}");
        }
    }

    fn spawn_notif_scrape() -> std::io::Result<()> {
        thread_manager::spawn_thread("scrape notifications", || {
            match schoology::notification::scrape_notifications() {
                Ok(mut notifs) => {
                    let last_sync = state().notif.last_sync;
                    for notif in notifs.iter_mut() {
                        notif.is_processed = notif.created < last_sync;
                    }
                    {
                        let notif_state = &mut state().notif;
                        notif_state.notifications = notifs.clone();
                        notif_state.save();
                        notif_state.last_update = Local::now();
                    }
                    log::info!("Finished scraping notifications");
                    if check_cancelled().is_err() {
                        return;
                    }
                    if let Err(err) = Self::update_notif_materials(notifs, Local::now()) {
                        log::warn!("Starting notification material update failed: {err}");
                        state().notif.is_checking_notifications = false;
                    }
                    Self::sync_calendar();
                }
                Err(e) => {
                    log::warn!("Scraping notifications failed: {e}");
                    state().notif.is_checking_notifications = false;
                    ui::sync_ui();
                }
            }
        })
        .map(|_| ())
    }

    fn update_notif_materials(
        mut notifs: Vec<Notification>,
        check_started: DateTime<Local>,
    ) -> std::io::Result<()> {
        thread_manager::spawn_thread("update notification materials", move || {
            let publish_progress = |progress| {
                crate::ui::run_on_ui_thread(move |ui| {
                    let global = ui.global::<crate::UiState>();
                    let mut notif = global.get_notif();
                    notif.progress = progress;
                    global.set_notif(notif);
                })
            };
            publish_progress(1.0);
            let result = schoology::notification::update::update(&mut notifs, publish_progress);
            Self::sync_calendar();
            let persisted = state().course.save();
            if let Err(err) = &result {
                log::warn!("Updating notification materials failed: {err}");
            }
            if let Err(err) = &persisted {
                log::warn!("Saving notification materials failed: {err}");
            }
            if result.is_ok() && persisted.is_ok() {
                // notifications arriving during sync are marked as not synced
                // the logic is here so that last_sync is only updated when sync is successful
                let mut state = state();
                state.notif.last_sync = check_started;
                state.notif.save();
            }
            state().notif.is_checking_notifications = false;
            ui::sync_ui();
        })
        .map(|_| ())
    }
    fn sync_calendar() {
        // Fetch without holding the app/config locks, then update only due dates.
        match schoology::calendar::fetch() {
            Ok(dates) => {
                let fetched = schoology::calendar::fetch_missing(&dates);
                let mut app = state();
                let updated = schoology::calendar::apply(&mut app.course.courses, &dates);
                if updated > 0 || fetched > 0 {
                    if let Err(err) = app.course.save() {
                        log::warn!("Saving calendar due dates failed: {err}");
                    }
                    log::info!(
                        "Fetched {fetched} assignments and updated {updated} due dates from calendar"
                    );
                }
            }
            Err(err) => log::warn!("Updating calendar due dates failed: {err}"),
        }
    }
    fn spawn_submission_sync() -> Result<(), std::io::Error> {
        let check_started = Local::now();

        thread_manager::spawn_thread("update submissions", move || {
            let mut assignments = {
                let app_state = state();
                app_state
                    .course
                    .walk_assignments()
                    .filter(|a| a.is_past_due() && !a.is_completed())
                    .cloned()
                    .collect::<Vec<_>>()
            };
            let _ = schoology::course::submissions::scrape_submissions(&mut assignments)
                .inspect_err(|err| log::warn!("Updating overdue submissions failed: {err}"));
            let mut submissions = assignments
                .into_iter()
                .map(|assignment| {
                    (
                        (assignment.course_id, assignment.id),
                        assignment.submissions,
                    )
                })
                .collect::<HashMap<_, _>>();
            {
                let mut s = state();
                for course in &mut s.course.courses {
                    for material in course.materials.recursive_iter_mut() {
                        if let Material::Assignment(assignment) = material {
                            if let Some(updated) = submissions
                                .remove(&(assignment.course_id.clone(), assignment.id.clone()))
                            {
                                assignment.submissions = updated;
                            }
                        }
                    }
                }
                s.notif.last_submission_sync = check_started;
                s.notif.is_checking_submissions = false;
                s.notif.save();
            }
            ui::sync_ui();
        })
        .map(|_| ())
    }
    pub fn sync_ui(&self, courses: &CourseState, ui: &AppWindow) {
        let notification_models = self
            .notifications
            .iter()
            .map(|notif| crate::Notification {
                title: notif.title.to_owned().into(),
                course: courses
                    .get_course(&notif.course_id)
                    .map_or("Unknown course", |c| c.course_title.as_str())
                    .into(),
                course_id: notif.course_id.to_owned().into(),
                n_type: match notif.material_type.unwrap_or(MaterialType::Folder) {
                    MaterialType::Assignment | MaterialType::Assessment => {
                        crate::NotificationType::NewAssignment
                    }
                    MaterialType::Document => crate::NotificationType::NewDocument,
                    MaterialType::Link => crate::NotificationType::NewLink,
                    _ => crate::NotificationType::Unknown,
                },
                icon_color: match notif.material_type.unwrap_or(MaterialType::Folder) {
                    MaterialType::Assignment | MaterialType::Assessment => {
                        ui.global::<UiState>().get_theme().accent_400
                    }
                    _ => ui.global::<UiState>().get_theme().text_400,
                },
            })
            .collect::<Vec<crate::Notification>>();
        ui.global::<crate::UiState>()
            .set_notif(crate::NotificationUi {
                progress: 0.0,
                temp_notifs: ModelRc::new(VecModel::from(Vec::new())), // todo!
                notifications: ModelRc::new(VecModel::from(notification_models)),
            });
    }
}
