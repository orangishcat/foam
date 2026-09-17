use std::sync::Mutex;

use crate::{AppWindow, state::state::state};

pub(super) static WEAK_UI: Mutex<Option<slint::Weak<AppWindow>>> = Mutex::new(None);

pub fn run_on_ui_thread(func: impl FnOnce(AppWindow) + Send + 'static) {
    let ui = WEAK_UI
        .lock()
        .expect("ui lock is poisoned")
        .as_ref()
        .expect("ui is not initialized")
        .clone();

    ui.upgrade_in_event_loop(func)
        .expect("event loop is not available");
}

pub fn sync_ui() {
    let snapshot = state().clone();
    run_on_ui_thread(move |ui| snapshot.sync_ui(&ui));
}
