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

#[cfg(test)]
mod tests;
