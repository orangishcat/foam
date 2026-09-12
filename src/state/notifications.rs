use chrono::Local;

use crate::{
    AppWindow,
    api::schoology,
    config::{config, config_write},
    state::courses::CourseState,
    types::notification::Notification,
};

#[derive(Default)]
pub struct NotificationState {
    pub notifications: Vec<Notification>,
}

impl NotificationState {
    pub fn check_notifications(&mut self) {
        if Local::now() - config().last_update < config().refresh_duration {
            return;
        }

        if let Ok(mut notifs) = schoology::notification::scrape_notifications() {
            let last_sync = config().last_sync;
            for notif in notifs.iter_mut() {
                notif.is_processed = notif.created < last_sync;
            }
            self.notifications = notifs;
        }
        config_write().last_update = Local::now();
    }
    pub fn sync_ui(&self, _courses: &CourseState, _ui: &slint::Weak<AppWindow>) {}
}
