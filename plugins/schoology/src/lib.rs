//! Schoology account: a native builtin "account plugin" for foam.
//!
//! The crate keeps the Schoology API/scraper/session code; all database
//! access goes through foam's core helpers while the plugin authorizer
//! scope is active (`foam::plugin::scoped("schoology")`), so its own
//! `plugin_schoology_*` tables and core-table writes follow the same
//! policy as wasm plugins. Secrets (cookies, API keys) stay inside this
//! crate's config structs and the `<data_dir>/plugins/schoology.json`
//! file created/edited by onboarding.

pub mod api;

use api::schoology::account::SchoologyAccountConfig;
use foam::account::Account;

/// Loaded for the accounts registry by the foam binary's native seam.
pub fn accounts_init() -> anyhow::Result<Vec<Box<dyn Account>>> {
    Ok(match SchoologyAccountConfig::load() {
        Ok(acc) => vec![Box::new(acc)],
        Err(err) => {
            log::warn!("Failed to read Schoology config: {err}");
            vec![]
        }
    })
}

pub use api::schoology::onboarding::{focus_input, init as onboarding_init};

/// Public resync helper for the native seam: clears the plugin's
/// full-fetch markers so the next fast+slow sync re-scrapes everything.
pub fn resync_data() -> anyhow::Result<()> {
    api::schoology::plugin_data::prepare_resync()
}

/// Resolve a Schoology notification through Schoology-owned metadata before
/// opening the corresponding generic Foam resource.
pub fn open_notification(ui: &foam::AppWindow, id: &str) -> bool {
    api::schoology::notification::open_notification(ui, id)
}

/// Schoology owns its notification icon mapping. Foam falls back to the
/// generic plugin icon when an event has no mapping.
pub fn notification_icon(event: &str) -> Option<String> {
    Some(match event {
        "grade_updated" => "circle-check",
        "resource_posted" => "file-plus",
        _ => return None,
    }
    .to_owned())
}

#[cfg(test)]
mod tests;
