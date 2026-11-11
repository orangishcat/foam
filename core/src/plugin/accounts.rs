//! Account registry: maps account ids to the plugin that provides them (or a
//! native builtin impl, e.g. Schoology).
//!
//! The plugin wasm boundary is described declaratively: a plugin declares its
//! accounts in the manifest (`accounts: [{id, label}]`), and the registry
//! aggregates them for the Accounts settings tab. Builtin native accounts are
//! listed alongside with a marker so the seams stay visible.

use serde::Serialize;

#[derive(Clone)]
#[derive(Serialize)]
pub struct AccountEntry {
    /// stable handle used by settings and future host-fn dispatch
    pub id: String,
    pub label: String,
    pub plugin_id: Option<String>,
    pub builtin: bool,
}

/// Activates the native accounts wired by the binary via the
/// `NativeAccounts.accounts_init` seam (`plugin::accounts::init_native`),
/// called once from `plugin::init()`. Without the seam (e.g. a build
/// without the Schoology feature) no native account exists and onboarding
/// opens with an empty registry.
pub fn init_native() -> anyhow::Result<()> {
    if let Some(init) = crate::app::native_accounts().map(|n| n.accounts_init) {
        crate::account::set_accounts(init()?);
    }
    Ok(())
}

pub fn entries() -> Vec<AccountEntry> {
    let mut accounts = Vec::new();
    if crate::app::native_accounts().is_some() {
        accounts.push(AccountEntry {
            id: "schoology".into(),
            label: "Schoology".into(),
            plugin_id: None,
            builtin: true,
        });
    }
    let plugins = crate::plugin::manifest::scan().unwrap_or_else(|error| {
        log::warn!("Scanning plugin manifests failed: {error}");
        Vec::new()
    });
    accounts.extend(plugins.iter().flat_map(|plugin| {
        plugin.accounts.iter().map(move |account| AccountEntry {
            id: format!("{}:{}", plugin.id, account.id),
            label: account.label.clone(),
            plugin_id: Some(plugin.id.clone()),
            builtin: false,
        })
    }));
    accounts
}

/// Dispatch notification actions to the plugin named in the notification's
/// source field. Builtin items stay in the core UI path.
pub fn open_notification(ui: &crate::AppWindow, source: &str, id: &str) -> bool {
    let Some(plugin_id) = source.strip_prefix("plugin:") else {
        return false;
    };
    if plugin_id == "schoology"
        && let Some(open) = crate::app::native_accounts().map(|accounts| accounts.open_notification)
    {
        return open(ui, id);
    }
    log::warn!("No notification handler registered for source {source}");
    false
}

/// Get a plugin-provided icon key for its opaque notification event. Unknown
/// or absent icons are handled by the core UI's generic plugin icon.
pub fn notification_icon(source: &str, event: &str) -> Option<String> {
    let plugin_id = source.strip_prefix("plugin:")?;
    if plugin_id == "schoology" {
        return crate::app::native_accounts()
            .and_then(|accounts| (accounts.notification_icon)(event));
    }
    None
}
