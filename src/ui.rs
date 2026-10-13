use slint::ComponentHandle;
use std::sync::Mutex;

use crate::{AppWindow, state::state::state};

pub(super) static WEAK_UI: Mutex<Option<slint::Weak<AppWindow>>> = Mutex::new(None);

#[derive(Default)]
struct ProgressPriority(Option<u8>);

impl ProgressPriority {
    fn accept(&mut self, progress: f32, priority: u8) -> bool {
        if progress == 0.0 && self.0 != Some(priority) {
            return false;
        }
        if self.0.is_some_and(|active| priority > active) {
            return false;
        }
        self.0 = (progress != 0.0).then_some(priority);
        true
    }
}

pub fn publish_progress(progress: f32, priority: u8, color: &slint::Color) {
    static PRIORITY: Mutex<ProgressPriority> = Mutex::new(ProgressPriority(None));
    let color = *color;
    run_on_ui_thread(move |ui| {
        if PRIORITY
            .lock()
            .expect("progress lock is poisoned")
            .accept(progress, priority)
        {
            let global = ui.global::<crate::NotificationUi>();
            global.set_progress_color(color);
            global.set_progress(progress);
        }
    });
}

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
