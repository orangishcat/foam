use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow,
    state::{courses::CourseState, dashboard::DashboardState, notifications::NotificationState},
};

#[derive(Default)]
pub struct AppState {
    pub courses: CourseState,
    pub dashboard: DashboardState,
    pub notifs: NotificationState,
}

impl AppState {
    pub fn init(&mut self) {
        self.courses.load_courses();
    }
    pub fn sync_ui(&mut self, ui: &slint::Weak<AppWindow>) {
        self.dashboard.sync_ui(&self.courses, ui);
        self.notifs.sync_ui(&self.courses, ui);
    }
    pub fn on_focus(&mut self) {
        self.notifs.check_notifications();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
