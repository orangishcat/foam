use serde::{Deserialize, Serialize};
use rusqlite::params;
use slint::winit_030::{EventResult, winit};use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

use crate::state::folders::FolderState;
use crate::{AppWindow, FolderTab, FoldersUi, Screen, UiState};
use anyhow;

#[derive(Clone, Default)]
pub struct SidebarState {}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedSidebar {
    selected_tab: i32,
    tabs: Vec<SavedTab>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedTab {
    folder_id: String,
    resource_folder_id: String,
    task_id: String,
    document_id: String,
}

impl SidebarState {
    pub fn restore_ui(&self, ui: &AppWindow) {
        let result = (|| -> anyhow::Result<()> {
            let saved = crate::database::from_sql_map(
                "SELECT data FROM sync_state WHERE key IN ('open_tabs', 'sidebar') ORDER BY CASE key WHEN 'open_tabs' THEN 0 ELSE 1 END".into(),
                &[],
                |row| {
                    let data: String = row.get(0)?;
                    Ok(serde_json::from_str::<SavedSidebar>(&data)?)
                },
            )?
            .into_iter()
            .next()
            .unwrap_or_default();
            let global = ui.global::<FoldersUi>();
            let tabs = saved
                .tabs
                .into_iter()
                .map(|tab| {
                    FolderState::folder_tab_image(tab.folder_id.clone());
                    FolderTab {
                        title: FolderState::folder_title(ui, &tab.folder_id),
                        folder_id: tab.folder_id.into(),
                        resource_folder_id: tab.resource_folder_id.into(),
                        task_id: tab.task_id.into(),
                        document_id: tab.document_id.into(),
                        ..Default::default()
                    }
                })
                .collect::<Vec<_>>();
            global.set_tabs(ModelRc::new(VecModel::from(tabs)));
            navigate_sidebar(ui, saved.selected_tab);
            crate::state::top_bar::restore_file_view(ui);
            Ok(())
        })();
        if let Err(err) = result {
            log::warn!("Restoring sidebar state failed: {err}");
        }
    }

    pub fn save_ui(&self, ui: &AppWindow) {
        let global = ui.global::<FoldersUi>();
        let tabs = global.get_tabs();
        let c_sidebar_item = current_sidebar_item(ui);
        log::debug!("current sidebar item: {}", c_sidebar_item);
        let saved = SavedSidebar {
            selected_tab: c_sidebar_item,
            tabs: tabs
                .iter()
                .map(|tab| SavedTab {
                    folder_id: tab.folder_id.to_string(),
                    resource_folder_id: tab.resource_folder_id.to_string(),
                    task_id: tab.task_id.to_string(),
                    document_id: tab.document_id.to_string(),
                })
                .collect(),
        };
        let result: anyhow::Result<()> = (|| {
            let data = serde_json::to_string(&saved)?;
            crate::database::execute(
                "INSERT INTO sync_state (key, data) VALUES ('open_tabs', ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data WHERE sync_state.data != excluded.data".into(),
                rusqlite::params![data],
            )?;
            Ok(())
        })();
        if let Err(err) = result {
            log::warn!("Saving sidebar state failed: {err}");
        }
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<FoldersUi>();
        if global
            .get_tabs()
            .as_any()
            .downcast_ref::<VecModel<FolderTab>>()
            .is_none()
        {
            global.set_tabs(ModelRc::from(Rc::new(VecModel::<FolderTab>::default())));
        }
        let weak = ui.as_weak();
        global.on_select_tab(move |index| {
            let Some(ui) = weak.upgrade() else { return };
            FolderState::select_tab(&ui, index);
        });
        let weak = ui.as_weak();
        global.on_close_tab(move |index| {
            if let Some(ui) = weak.upgrade() {
                FolderState::close_tab(&ui, index);
            }
        });
        let weak = ui.as_weak();
        global.on_open(move |folder| {
            let Some(ui) = weak.upgrade() else { return };
            match FolderState::ensure_folder_tab(&ui, &folder, None) {
                Ok(index) => FolderState::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening folder failed: {err}"),
            }
        });
        let weak = ui.as_weak();
        global.on_open_with_image(move |folder, icon| {
            let Some(ui) = weak.upgrade() else { return };
            match FolderState::ensure_folder_tab(&ui, &folder, Some(icon)) {
                Ok(index) => FolderState::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening folder failed: {err}"),
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

        // plugin-pinned items under the builtin divider; click dispatch is a
        // seam until the plugin tab system lands
        let pins: Vec<crate::SidebarPin> = crate::plugin::pinned_tabs()
            .into_iter()
            .map(|tab| {
                let icon = tab
                    .icon
                    .filter(|path| !path.is_empty())
                    .and_then(|path| {
                        slint::Image::load_from_path(std::path::Path::new(&path))
                            .inspect_err(|err| {
                                log::warn!("failed to load pin icon {path}: {err}")
                            })
                            .ok()
                    });
                crate::SidebarPin {
                    kind: "tab".into(),
                    id: tab.id.into(),
                    title: tab.title.into(),
                    icon: icon.unwrap_or_default(),
                }
            })
            .collect();
        ui.global::<UiState>().set_plugin_pins(ModelRc::new(VecModel::from(pins)));
        let weak = ui.as_weak();
        ui.global::<UiState>().on_plugin_pin_clicked(move |pin| {
            let Some(ui) = weak.upgrade() else { return };
            // cheap dispatch: a pin whose id maps to a folder opens that
            // folder's tab; anything else is unsupported for now
            let exists: bool = crate::database::from_sql_map(
                "SELECT EXISTS(SELECT 1 FROM folders WHERE folder_id = ?1)".into(),
                params![pin.id.to_string()],
                |row| Ok(row.get::<_, bool>(0)?),
            )
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or(false);
            if exists {
                match FolderState::ensure_folder_tab(&ui, &pin.id.to_string(), None) {
                    Ok(index) => FolderState::select_tab(&ui, index as i32),
                    Err(err) => log::warn!("Opening pinned folder tab failed: {err}"),
                }
            } else {
                log::info!(
                    "plugin pin clicked but unsupported: kind={}, id={}",
                    pin.kind,
                    pin.id
                );
            }
        });
        let weak = ui.as_weak();
        ui.on_step_sidebar(move |delta| {
            if let Some(ui) = weak.upgrade() {
                let count = ui.global::<FoldersUi>().get_tabs().row_count() as i32;
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
    let folders = ui.global::<FoldersUi>();
    let state = ui.global::<UiState>();
    let count = folders.get_tabs().row_count() as i32;
    let focused = state.get_focused_sidebar_item();
    if focused >= 0 {
        return focused;
    }
    match state.get_screen() {
        Screen::Calendar => 0,
        Screen::Notifs => 1,
        Screen::Folders => 2,
        Screen::Search => 3,
        Screen::Resources | Screen::Task => folders.get_active_tab() + 4,
        Screen::Settings => count + 4,
        _ => 0,
    }
}

fn navigate_sidebar(ui: &AppWindow, target: i32) {
    if ui.global::<UiState>().get_screen() == Screen::Onboarding {
        return;
    }
    let folders = ui.global::<FoldersUi>();
    let state = ui.global::<UiState>();
    let count = folders.get_tabs().row_count() as i32;
    if target < 0 || target >= count + 5 {
        return;
    }
    match target {
        0 => state.set_screen(Screen::Calendar),
        1 => state.set_screen(Screen::Notifs),
        2 => state.set_screen(Screen::Folders),
        3 => state.set_screen(Screen::Search),
        n if n == count + 4 => state.set_screen(Screen::Settings),
        n => folders.invoke_select_tab(n - 4),
    }
    ui.set_sidebar_focus_request(-1);
    ui.set_sidebar_focus_request(target);
}
