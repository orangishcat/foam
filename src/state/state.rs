use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow,
    state::{dashboard::DashboardState, notifications::NotificationState},
};

#[derive(Clone, Default)]
pub struct AppState {
    pub dashboard: DashboardState,
    pub notif: NotificationState,
}

impl AppState {
    pub fn init(&mut self) {
        self.notif.load();
    }
    pub fn sync_ui(&self, ui: &AppWindow) {
        if let Err(err) = self.dashboard.sync_ui(ui) {
            log::warn!("Loading dashboard failed: {err}");
        }
        if let Err(err) = self.notif.sync_ui(ui) {
            log::warn!("Loading notifications failed: {err}");
        }
    }
    pub fn on_focus(&mut self) {
        self.notif.check_notifications();
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}
