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
}
