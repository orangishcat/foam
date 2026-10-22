use slint::ComponentHandle;
use std::io::Error;
use std::sync::Mutex;

use crate::{AppWindow, UiState, database, state::state::state};

pub(crate) fn progress_color(ui: &AppWindow, percent: f64) -> slint::Color {
    let theme = ui.global::<UiState>().get_theme();
    let level = (((percent / 100.0) * 9.0).floor() as usize).clamp(1, 9);
    match level {
        1 => theme.progress_100,
        2 => theme.progress_200,
        3 => theme.progress_300,
        4 => theme.progress_400,
        5 => theme.progress_500,
        6 => theme.progress_600,
        7 => theme.progress_700,
        8 => theme.progress_800,
        _ => theme.progress_900,
    }
}

pub(crate) fn progress_percent(course: &str, id: &str) -> Option<f64> {
    database::from_sql_map(
        "SELECT score, max_points FROM assignments WHERE course_id = ? AND id = ?".into(),
        rusqlite::params![course, id],
        |row| {
            Ok((
                row.get::<_, Option<f64>>(0).map_err(Error::other)?,
                row.get::<_, f64>(1).map_err(Error::other)?,
            ))
        },
    )
    .ok()
    .and_then(|rows| rows.into_iter().next())
    .and_then(|(score, max_points)| {
        (max_points > 0.0)
            .then(|| score.map(|score| score / max_points * 100.0))
            .flatten()
    })
}

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
