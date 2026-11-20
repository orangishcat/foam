use std::time::Duration;

use cargo_packager_updater::{Config, semver::Version};
use reqwest::Url;
use slint::ComponentHandle;

use crate::{UiState, ui};
use anyhow::{self, Ok};

pub fn spawn_check_update_timer() {
    check_update_ui();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_hours(24),
        check_update_ui,
    );
}

pub fn check_update_ui() {
    if let Some(has_update) = check_update()
        .inspect(|u| log::info!("Is newer version available: {u}"))
        .inspect_err(|e| log::warn!("Failed to check updates: {e}"))
        .ok()
        && has_update
    {
        ui::run_on_ui_thread(|ui| {
            ui.global::<UiState>()
                .set_top_right_text("Update available, restart to apply".into());
        });
    }
}

pub fn check_update() -> anyhow::Result<bool> {
    let config = Config {
        endpoints: vec![Url::parse(
            "https://github.com/orangishcat/foam/releases/latest/download/latest.json",
        )?],
        pubkey: include_str!("foam.key.pub").into(),
        ..Default::default()
    };

    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    if let Some(update) = cargo_packager_updater::check_update(current, config)? {
        update.download_and_install()?;
        Ok(true)
    } else {
        Ok(false)
    }
}
