use std::io;

use slint::ComponentHandle;

use crate::{
    AppWindow, SettingsUi, UiState,
    config::{config, config_write},
};

#[derive(Clone, Default)]
pub struct SettingsState {}

impl SettingsState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let font_size = config().font_size.clamp(12, 24);
        ui.global::<UiState>().set_font_size(font_size as f32);
        ui.global::<SettingsUi>().set_font_size(font_size);
        let weak = ui.as_weak();
        ui.global::<SettingsUi>().on_set_font_size(move |size| {
            let Some(ui) = weak.upgrade() else { return };
            let size = size.clamp(12, 24);
            ui.global::<UiState>().set_font_size(size as f32);
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
