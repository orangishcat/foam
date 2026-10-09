use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow, Screen, UiState,
    state::{
        assignment_view::AssignmentViewState, courses::CourseState, dashboard::DashboardState,
        document_view::DocumentViewerState, notifications::NotificationState, search::SearchState,
        settings::SettingsState,
    },
};

use slint::ComponentHandle;

#[derive(Clone, Default)]
pub struct AppState {
    pub courses: CourseState,
    pub assignment_view: AssignmentViewState,
    pub dashboard: DashboardState,
    pub notif: NotificationState,
    pub document_view: DocumentViewerState,
    pub settings: SettingsState,
    pub search: SearchState,
}

impl AppState {
    pub fn init(&mut self) {
        self.notif.init();
    }
    pub fn init_ui(&self, ui: &AppWindow) {
        self.courses.init_ui(ui);
        self.assignment_view.init_ui(ui);
        self.document_view.init_ui(ui);
        self.notif.init_ui(ui);
        self.search.init_ui(ui);
        ui.global::<UiState>()
            .set_font_size(crate::config::config().font_size.clamp(12, 24) as f32);
        ui.on_screen_changed(crate::ui::sync_ui);
    }

    pub fn sync_ui(&self, ui: &AppWindow) {
        let screen = ui.global::<UiState>().get_screen();
        if matches!(
            screen,
            Screen::Courses | Screen::Materials | Screen::Assignment | Screen::Document
        ) {
            if let Err(err) = self.courses.sync_ui(ui) {
                log::warn!("courses sync_ui failed: {err}");
            }
        }
        let result = match screen {
            Screen::Dashboard => self.dashboard.sync_ui(ui),
            Screen::Notifs => self.notif.sync_ui(ui),
            Screen::Assignment => self.assignment_view.sync_ui(ui),
            Screen::Document => self.document_view.sync_ui(ui),
            Screen::Settings => self.settings.sync_ui(ui),
            Screen::Search => self.search.sync_ui(ui),
            Screen::Courses | Screen::Materials | Screen::Onboarding => Ok(()),
        };
        if let Err(err) = result {
            log::warn!("active screen sync_ui failed: {err}");
        }
        if ui.global::<UiState>().get_screen() != Screen::Notifs {
            if let Err(err) = self.notif.sync_sidebar(ui) {
                log::warn!("notification sidebar sync failed: {err}");
            }
        }
    }
    pub fn on_focus(&mut self) {
        self.notif.check_notifications();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
