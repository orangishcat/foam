use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow,
    state::{courses::CourseState, dashboard::DashboardState, notifications::NotificationState},
};

#[derive(Default)]
pub struct AppState {
    pub course: CourseState,
    pub dashboard: DashboardState,
    pub notif: NotificationState,
}

impl AppState {
    pub fn init(&mut self) {
        self.notif.load();
    }
    pub fn sync_ui(&mut self, ui: &AppWindow) {
        self.dashboard.sync_ui(ui);
        self.notif.sync_ui(ui);
    }
    pub fn on_focus(&mut self) {
        self.notif.check_notifications();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
