use chrono::Local;

use crate::{
    AppWindow,
    api::schoology,
    config::{config, config_write},
    state::{courses::CourseState, state::state},
    thread_manager,
    types::notification::Notification,
};

#[derive(Default)]
pub struct NotificationState {
    pub notifications: Vec<Notification>,
    is_checking_notifications: bool,
}

impl NotificationState {
    pub fn check_notifications(&mut self) {
        if Local::now() - config().last_update < config().refresh_duration
            || self.is_checking_notifications
        {
            return;
        }

        self.is_checking_notifications = true;

        if let Err(err) = thread_manager::spawn_thread("scrape notifications", || {
            if let Ok(mut notifs) = schoology::notification::scrape_notifications() {
                let last_sync = config().last_sync;
                for notif in notifs.iter_mut() {
                    notif.is_processed = notif.created < last_sync;
                }
                state().notifs.notifications = notifs;
                state().notifs.is_checking_notifications = false;
            }
            config_write().last_update = Local::now();
            log::info!("Finished scraping notifications");
        }) {
            log::warn!("Scraping notifications failed: {err}");
        }
    }
    pub fn sync_ui(&self, _courses: &CourseState, _ui: &AppWindow) {}
}
