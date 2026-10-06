use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow,
    state::{
        dashboard::DashboardState, document_view::DocumentViewerState,
        notifications::NotificationState,
    },
};

#[derive(Clone, Default)]
pub struct AppState {
    pub dashboard: DashboardState,
    pub notif: NotificationState,
    pub document_view: DocumentViewerState,
}

impl AppState {
    pub fn init(&mut self) {
        self.notif.init();
    }
    pub fn sync_ui(&self, ui: &AppWindow) {
        crate::state::assignment_view::init(ui);
        if let Err(err) = crate::state::courses::sync_ui(ui) {
            log::warn!("courses sync_ui failed: {err}");
        }
        if let Err(err) = self.dashboard.sync_ui(ui) {
            log::warn!("dashboard sync_ui failed: {err}");
        }
        if let Err(err) = self.notif.sync_ui(ui) {
            log::warn!("notifications sync_ui failed: {err}");
        }
        if let Err(err) = self.document_view.sync_ui(ui) {
            log::warn!("document_view sync_ui failed: {err}");
        }
    }
    pub fn on_focus(&mut self) {
        self.notif.check_notifications();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
