use std::time::Duration;

use cargo_packager_updater::{Config, semver::Version};
use chrono::{DateTime, Local, TimeDelta};
use reqwest::Url;
use rusqlite::params;
use slint::ComponentHandle;

use crate::{UiState, database, ui};
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
    database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = 'last_update_check'".to_owned(),
        &[],
        |row| {
            let data: String = row.get(0)?;
            Ok(DateTime::parse_from_rfc3339(&data)?.with_timezone(&Local))
        },
    ).and_then(|value| {
        if value.is_empty() || Local::now() - value[0] < TimeDelta::from_std(Duration::from_hours(4))? {
            log::debug!("Update check fresh, skipping");
            return Ok(());
        }
        log::debug!("Checking for new update");

        database::execute("INSERT INTO sync_state (key, data) VALUES ('last_update_check', ?) ON CONFLICT(key) DO NOTHING".to_owned(), params![Local::now().to_rfc3339()])?;

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
        Ok(())
    }).inspect_err(|e| log::warn!("Updater check failed: {e}")).ok();
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
