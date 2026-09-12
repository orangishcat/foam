use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow,
    state::{courses::CourseState, dashboard::DashboardState},
};

#[derive(Default)]
pub struct AppState {
    pub courses: CourseState,
    pub dashboard: DashboardState,
}

impl AppState {
    pub fn init(&mut self) {
        self.courses.load_courses();
        self.courses.check_and_refresh_courses();
    }
    pub fn sync_ui(&self, ui: &AppWindow) {
        self.dashboard.sync_ui(&self.courses, ui);
    }
    pub fn on_focus(&mut self) {
        self.courses.check_and_refresh_courses();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
