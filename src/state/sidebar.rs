use serde::{Deserialize, Serialize};
use slint::winit_030::{EventResult, winit};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

use crate::state::courses::CourseState;
use crate::{AppWindow, CourseTab, CoursesUi, Screen, UiState};

#[derive(Clone, Default)]
pub struct SidebarState {}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedSidebar {
    selected_tab: Option<usize>,
    tabs: Vec<SavedTab>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedTab {
    course_id: String,
    folder_id: String,
    assignment_id: String,
    document_id: String,
}

impl SidebarState {
    pub fn restore_ui(&self, ui: &AppWindow) {
        let result = (|| -> std::io::Result<()> {
            let saved = crate::database::from_sql_map(
                "SELECT data FROM sync_state WHERE key = 'sidebar'".into(),
                &[],
                |row| {
                    let data: String = row.get(0).map_err(std::io::Error::other)?;
                    serde_json::from_str::<SavedSidebar>(&data).map_err(std::io::Error::other)
                },
            )?
            .into_iter()
            .next()
            .unwrap_or_default();
            let global = ui.global::<CoursesUi>();
            let tabs = saved
                .tabs
                .into_iter()
                .map(|tab| CourseTab {
                    title: CourseState::course_title(ui, &tab.course_id),
                    course_id: tab.course_id.into(),
                    folder_id: tab.folder_id.into(),
                    assignment_id: tab.assignment_id.into(),
                    document_id: tab.document_id.into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            let selected = saved
                .selected_tab
                .filter(|&index| index < tabs.len())
                .or_else(|| (!tabs.is_empty()).then_some(0));
            global.set_tabs(ModelRc::new(VecModel::from(tabs)));
            for index in 0..global.get_tabs().row_count() {
                let tab = global.get_tabs().row_data(index).unwrap();
                global.set_active_tab(index as i32);
                global.set_course_id(tab.course_id.clone());
                global.set_folder_id(tab.folder_id);
                crate::state::top_bar::restore_file_view(ui);
                CourseState::course_tab_image(tab.course_id.to_string());
            }
            global.set_active_tab(selected.map(|index| index as i32).unwrap_or(-1));
            if let Some(tab) = selected.and_then(|index| global.get_tabs().row_data(index)) {
                global.set_course_id(tab.course_id);
                global.set_folder_id(tab.folder_id);
            }
            crate::state::top_bar::restore_file_view(ui);
            Ok(())
        })();
        if let Err(err) = result {
            log::warn!("Restoring sidebar state failed: {err}");
        }
    }

    pub fn save_ui(ui: &AppWindow) {
        let global = ui.global::<CoursesUi>();
        let tabs = global.get_tabs();
        let saved = SavedSidebar {
            selected_tab: usize::try_from(global.get_active_tab())
                .ok()
                .filter(|&index| index < tabs.row_count()),
            tabs: tabs
                .iter()
                .map(|tab| SavedTab {
                    course_id: tab.course_id.to_string(),
                    folder_id: tab.folder_id.to_string(),
                    assignment_id: tab.assignment_id.to_string(),
                    document_id: tab.document_id.to_string(),
                })
                .collect(),
        };
        let result = serde_json::to_string(&saved).map_err(std::io::Error::other)
            .and_then(|data| crate::database::execute(
                "INSERT INTO sync_state (key, data) VALUES ('sidebar', ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data WHERE sync_state.data != excluded.data".into(),
                rusqlite::params![data],
            ).map(|_| ()));
        if let Err(err) = result {
            log::warn!("Saving sidebar state failed: {err}");
        }
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<CoursesUi>();
        if global
            .get_tabs()
            .as_any()
            .downcast_ref::<VecModel<CourseTab>>()
            .is_none()
        {
            global.set_tabs(ModelRc::from(Rc::new(VecModel::<CourseTab>::default())));
        }
        let weak = ui.as_weak();
        global.on_select_tab(move |index| {
            let Some(ui) = weak.upgrade() else { return };
            CourseState::select_tab(&ui, index);
        });
        let weak = ui.as_weak();
        global.on_close_tab(move |index| {
            if let Some(ui) = weak.upgrade() {
                CourseState::close_tab(&ui, index);
            }
        });
        let weak = ui.as_weak();
        global.on_open(move |course| {
            let Some(ui) = weak.upgrade() else { return };
            match CourseState::ensure_course_tab(&ui, &course, None) {
                Ok(index) => CourseState::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening course failed: {err}"),
            }
        });
        let weak = ui.as_weak();
        global.on_open_with_image(move |course, icon| {
            let Some(ui) = weak.upgrade() else { return };
            match CourseState::ensure_course_tab(&ui, &course, Some(icon)) {
                Ok(index) => CourseState::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening course failed: {err}"),
            }
        });
    }
}

pub struct Sidebar {
    ui: slint::Weak<AppWindow>,
    modifiers: winit::keyboard::ModifiersState,
}

impl Sidebar {
    pub fn new(ui: &AppWindow) -> Self {
        ui.global::<UiState>().set_shortcut_modifier(
            if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Ctrl"
            }
            .into(),
        );
        let weak = ui.as_weak();
        ui.on_navigate_sidebar(move |target| {
            if let Some(ui) = weak.upgrade() {
                navigate_sidebar(&ui, target);
            }
        });
        let weak = ui.as_weak();
        ui.on_step_sidebar(move |delta| {
            if let Some(ui) = weak.upgrade() {
                let count = ui.global::<CoursesUi>().get_tabs().row_count() as i32;
                navigate_sidebar(
                    &ui,
                    (current_sidebar_item(&ui) + delta).rem_euclid(count + 4),
                );
            }
        });
        Self {
            ui: ui.as_weak(),
            modifiers: winit::keyboard::ModifiersState::empty(),
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> EventResult {
        let Some(ui) = self.ui.upgrade() else {
            return EventResult::Propagate;
        };
        match event {
            winit::event::WindowEvent::ModifiersChanged(value) => {
                self.modifiers = value.state();
                ui.global::<UiState>()
                    .set_shift_held(self.modifiers.shift_key());
            }
            winit::event::WindowEvent::Focused(false) => {
                self.modifiers = winit::keyboard::ModifiersState::empty();
                ui.global::<UiState>().set_shift_held(false);
            }
            _ => {}
        }
        EventResult::Propagate
    }
}

fn current_sidebar_item(ui: &AppWindow) -> i32 {
    let courses = ui.global::<CoursesUi>();
    let state = ui.global::<UiState>();
    let count = courses.get_tabs().row_count() as i32;
    let focused = state.get_focused_sidebar_item();
    if focused >= 0 {
        return focused;
    }
    match state.get_screen() {
        Screen::Dashboard => 0,
        Screen::Notifs => 1,
        Screen::Courses => 2,
        Screen::Search => 3,
        Screen::Materials | Screen::Assignment => courses.get_active_tab() + 4,
        Screen::Settings => count + 4,
        _ => 0,
    }
}

fn navigate_sidebar(ui: &AppWindow, target: i32) {
    if ui.global::<UiState>().get_screen() == Screen::Onboarding {
        return;
    }
    let courses = ui.global::<CoursesUi>();
    let state = ui.global::<UiState>();
    let count = courses.get_tabs().row_count() as i32;
    if target < 0 || target >= count + 5 {
        return;
    }
    match target {
        0 => state.set_screen(Screen::Dashboard),
        1 => state.set_screen(Screen::Notifs),
        2 => state.set_screen(Screen::Courses),
        3 => state.set_screen(Screen::Search),
        n if n == count + 4 => state.set_screen(Screen::Settings),
        n => courses.invoke_select_tab(n - 4),
    }
    ui.set_sidebar_focus_request(-1);
    ui.set_sidebar_focus_request(target);
}
