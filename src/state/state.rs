
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
    pub fn sync_ui(&self, ui: &AppWindow) {
        self.dashboard.sync_ui(&self.courses, ui);
    }
    pub fn on_focus(&mut self, _ui: &AppWindow) {
        self.courses.check_and_refresh_courses();
    }
}
