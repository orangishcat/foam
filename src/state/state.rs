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
    pub fn sync_ui(&mut self, ui: &AppWindow) {
        self.dashboard.ui.set_assignment_view(value);
    }
}
