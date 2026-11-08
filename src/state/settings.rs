use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel};

use crate::state::state::State;
use crate::{
    AppWindow, SettingsUi, UiState,
    config::{config, config_write},
    theme::load_themes,
};
use anyhow;

#[derive(Clone, Default)]
pub struct SettingsState {}

impl State for SettingsState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let font_size = config().font_size.clamp(12, 24);
        ui.global::<UiState>().set_font_size(font_size);
        ui.global::<SettingsUi>().set_font_size(font_size);
        let weak = ui.as_weak();
        ui.global::<SettingsUi>().on_set_font_size(move |size| {
            let Some(ui) = weak.upgrade() else { return };
            let size = size.clamp(4, 24);
            ui.global::<UiState>().set_font_size(size);
            ui.global::<SettingsUi>().set_font_size(size);
            let mut config = config_write();
            config.font_size = size;
            let error = config
                .save()
                .err()
                .map(|e| format!("Could not save font size: {e}"))
                .unwrap_or_default();
            ui.global::<SettingsUi>().set_save_error(error.into());
        });
        Ok(())
    }
}

impl SettingsState {
    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<UiState>();
        let themes = match load_themes(
            config().data_dir(),
            global.get_default_dark_theme(),
            global.get_default_light_theme(),
        ) {
            Ok(themes) => themes,
            Err(error) => {
                log::warn!("Could not load themes: {error}");
                return;
            }
        };
        let selected_name = config().selected_theme.clone();
        let selected = themes
            .iter()
            .position(|theme| theme.name == selected_name)
            .unwrap_or(0);
        global.set_theme(themes[selected].palette.clone());
        let settings = ui.global::<SettingsUi>();
        settings.set_themes(ModelRc::new(VecModel::from(
            themes
                .iter()
                .map(|theme| theme.name.to_shared_string())
                .collect::<Vec<_>>(),
        )));
        settings.set_selected_theme(selected as i32);

        let weak = ui.as_weak();
        settings.on_select_theme(move |index| {
            let Some(ui) = weak.upgrade() else { return };
            let Some(theme) = usize::try_from(index).ok().and_then(|i| themes.get(i)) else {
                return;
            };
            ui.global::<UiState>().set_theme(theme.palette.clone());
            let mut config = config_write();
            config.selected_theme = theme.name.clone();
            let error = config
                .save()
                .err()
                .map(|e| format!("Could not save theme: {e}"))
                .unwrap_or_default();
            ui.global::<SettingsUi>().set_save_error(error.into());
        });
    }
}
