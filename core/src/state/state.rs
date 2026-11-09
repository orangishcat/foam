use anyhow;
use std::sync::{LazyLock, Mutex, MutexGuard};

static STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::default()));

use crate::{
    AppWindow, AttachmentMenu, MenubarData, Screen, ScreenMenuItem, UiState,
    config::config,
    state::{
        attachment_view,
        calendar::CalendarState,
        document_view::DocumentViewerState,
        folders::FolderState,
        notifications::NotificationState,
        resources::ResourcesState,
        search::SearchState,
        settings::SettingsState,
        sidebar::SidebarState,
        task_view::TaskViewState,
    },
};

use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel};

#[derive(Clone, Default)]
pub struct AppState {
    pub folders: FolderState,
    pub sidebar: SidebarState,
    pub resources: ResourcesState,
    pub task_view: TaskViewState,
    pub calendar: CalendarState,
    pub notif: NotificationState,
    pub document_view: DocumentViewerState,
    pub settings: SettingsState,
    pub search: SearchState,
}

pub trait State {
    fn sync_ui(&self, _ui: &AppWindow) -> anyhow::Result<()> {
        Ok(())
    }
    fn menu_items(&self, _ui: &AppWindow) -> Vec<ScreenMenuItem> {
        Vec::new()
    }
}

impl AppState {
    fn active_screen_state(&self, screen: Screen) -> Option<&dyn State> {
        match screen {
            Screen::Calendar => Some(&self.calendar),
            Screen::Notifs => Some(&self.notif),
            Screen::Folders => Some(&self.folders),
            Screen::Resources => Some(&self.resources),
            Screen::Task => Some(&self.task_view),
            Screen::Document => Some(&self.document_view),
            Screen::Search => Some(&self.search),
            Screen::Settings => Some(&self.settings),
            Screen::Onboarding => None,
        }
    }
    pub fn init(&mut self) {
        self.notif.init();
    }
    pub fn init_ui(&self, ui: &AppWindow) {
        self.sidebar.init_ui(ui);
        self.resources.init_ui(ui);
        self.folders.init_ui(ui);
        self.task_view.init_ui(ui);
        self.document_view.init_ui(ui);
        self.notif.init_ui(ui);
        self.search.init_ui(ui);
        self.sidebar.restore_ui(ui);
        self.settings.init_ui(ui);
        ui.global::<UiState>()
            .set_font_size(config().font_size.clamp(8, 24));
        ui.global::<MenubarData>().on_refresh(|| {
            let snapshot = state().clone();
            if let Some(ui) = crate::ui::strong_ui() {
                snapshot.sync_menubar(&ui);
            }
        });
        self.sync_menubar(ui);
        ui.on_screen_changed(crate::ui::sync_ui);
    }

    pub fn sync_ui(&self, ui: &AppWindow) {
        let screen = ui.global::<UiState>().get_screen();
        if let Some(screen_state) = self.active_screen_state(screen)
            && let Err(err) = screen_state.sync_ui(ui)
        {
            log::warn!("active screen sync_ui failed: {err}");
        }
        self.sync_menubar(ui);
    }
    pub fn save(&mut self, ui: &AppWindow) {
        self.sidebar.save_ui(ui);
    }
    pub fn on_focus(&mut self) {
        self.notif.check_notifications();
    }
    pub fn sync_menubar(&self, ui: &AppWindow) {
        let screen = ui.global::<UiState>().get_screen();
        let items = if ui.global::<AttachmentMenu>().get_visible() {
            attachment_view::menu_items(ui)
        } else {
            match self.active_screen_state(screen) {
                Some(screen_state) => screen_state.menu_items(ui),
                None => Vec::new(),
            }
        };
        ui.global::<MenubarData>()
            .set_items(ModelRc::new(VecModel::from(items)));
    }
}

pub fn state() -> MutexGuard<'static, AppState> {
    STATE.lock().expect("state lock is poisoned")
}

/// Build a menu item for the menubar, with the shortcut written the same way
/// as the `.slint` `@keys` macro expects (`Control` + `R` => `&["Control", "R"]`).
pub(crate) fn menu_item(title: &str, keys: &[&str], enabled: bool) -> ScreenMenuItem {
    ScreenMenuItem {
        title: title.to_shared_string(),
        shortcut: slint::Keys::from_parts(keys.iter().copied()).unwrap_or_default(),
        enabled,
    }
}
