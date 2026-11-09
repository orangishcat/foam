//! Plugin registry: installs manifests on disk into extism wasm plugins and
//! provides lifecycle + invoke entry points.

pub mod accounts;
pub mod host;
pub mod manifest;
#[cfg(test)]
mod tests;

use std::sync::{LazyLock, Mutex};

use anyhow;
use extism::{Manifest, Plugin, PluginBuilder, Wasm};
// dead_code until the plugin dispatch (Phase 3/4 syncs) consumes `call`
#[allow(dead_code)]
struct RegisteredPlugin {
    id: String,
    plugin: Plugin,
}

static REGISTRY: LazyLock<Mutex<Vec<RegisteredPlugin>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Takes the runtime lock: plugin execution is serialized so host fns and the
/// authorizer agree on which plugin is calling (see `plugin/host.rs`).
// dead_code until the plugin dispatch (Phase 3/4 syncs) consumes `call`
#[allow(dead_code)]
fn registry() -> std::sync::MutexGuard<'static, Vec<RegisteredPlugin>> {
    REGISTRY.lock().unwrap_or_else(|err| err.into_inner())
}

fn load(manifest: &manifest::PluginManifest) -> anyhow::Result<Plugin> {
    let extism_manifest =
        Manifest::new([Wasm::file(manifest::plugins_dir().join(format!("{}.wasm", manifest.id)))])
            .disallow_all_hosts()
            .with_config(
                [(
                    "config".to_string(),
                    serde_json::to_string(&manifest.config).unwrap_or_default(),
                )]
                .into_iter(),
            );
    PluginBuilder::new(extism_manifest)
        .with_wasi(true)
        .with_functions(host::host_functions())
        .build()
        .map_err(|err| anyhow::anyhow!("building plugin {}: {err}", manifest.id))
}

/// Scans `<data_dir>/plugins` and builds the extism registry, calling
/// `on_enabled` / `init` exports when present. Errors are logged, never fatal:
/// a broken plugin must not stop the app from booting.
pub fn init() -> anyhow::Result<()> {
    accounts::init_native()?;
    let plugins = manifest::scan()?;
    let mut registry = REGISTRY
        .lock()
        .map_err(|_| anyhow::anyhow!("plugin registry is poisoned"))?;
    for entry in plugins {
        match load(&entry) {
            Ok(mut plugin) => {
                // lifecycle: invoke exports if the plugin exposes them; a
                // missing or failing export is skipped silently. The active
                // plugin id rides these calls so the authorizer scopes the
                // plugin's own SQL.
                for export in ["on_enabled", "init"] {
                    if plugin.function_exists(export) {
                        host::set_active_plugin(Some(entry.id.clone()));
                        let result =
                            plugin.call::<&str, &str>(export, "").map(|_| ());
                        host::set_active_plugin(None);
                        if let Err(error) = result {
                            log::warn!("plugin {} export {export} failed: {error}", entry.id);
                        }
                    }
                }
                if let Some(search) = &entry.search {
                    if let Err(error) = host::apply_search_mapping(&entry.id, search) {
                        log::warn!("plugin {} search mapping failed: {error}", entry.id);
                    }
                } else if let Err(error) = host::clear_mapped_search(&entry.id) {
                    log::warn!("clearing plugin {} search rows failed: {error}", entry.id);
                }
                registry.push(RegisteredPlugin {
                    id: entry.id.clone(),
                    plugin,
                });
                log::info!("loaded plugin {}", entry.id);
            }
            Err(error) => {
                log::warn!("failed to load plugin {}: {error}", entry.id);
            }
        }
    }
    Ok(())
}

/// Scoped native execution: while the returned guard lives, the authorizer
/// attributes every DB access through `database::connection()` to
/// `plugin_id` — exactly what the wasm `plugin_db_*` host fns do for
/// out-of-process plugins. Native builtin plugins (e.g. Schoology) wrap
/// their sync entry points in this guard so their `plugin_<id>_ *` tables
/// and core-table writes follow the same policy as plugins.
///
/// Nested scopes restore the previous active plugin on drop.
pub struct PluginScope {
    previous: Option<String>,
    id: String,
}

impl PluginScope {
    pub fn new(plugin_id: &str) -> Self {
        let previous = host::active_plugin();
        host::set_active_plugin(Some(plugin_id.to_owned()));
        PluginScope {
            previous,
            id: plugin_id.to_owned(),
        }
    }
}

impl Drop for PluginScope {
    fn drop(&mut self) {
        log::trace!("releasing plugin scope {}", self.id);
        host::set_active_plugin(self.previous.clone());
    }
}

pub fn scoped(plugin_id: &str) -> PluginScope {
    PluginScope::new(plugin_id)
}

/// Invoke the export `name` of plugin `id` with an empty input, returning its
/// output as a UTF-8 string.
///
/// While the call runs, the authorizer associates all DB access from this
/// plugin's host fns with `id` and the plugin runtime mutex prevents
/// interleaving calls.
// dead_code until the plugin dispatch (Phase 3/4 syncs) consumes `call`
#[allow(dead_code)]
pub fn call(id: &str, name: &str) -> anyhow::Result<String> {
    let mut registry = registry();
    let plugin = registry
        .iter_mut()
        .find(|entry| entry.id == id)
        .map(|entry| &mut entry.plugin)
        .ok_or_else(|| anyhow::anyhow!("unknown plugin {id}"))?;
    host::set_active_plugin(Some(id.to_owned()));
    let outcome = plugin.call::<&str, &str>(name, "");
    // no plugin call may outlive its active id; clear before returning
    host::set_active_plugin(None);
    outcome
        .map(|output| output.to_owned())
        .map_err(|err| anyhow::anyhow!("plugin {id} export {name}: {err}"))
}

/// Pinned sidebar tab descriptors from all loaded plugins, for the seam in
/// `ui/components/sidebar.slint` (rendered under a divider below builtins).
pub fn pinned_tabs() -> Vec<manifest::PinnedTab> {
    manifest::scan()
        .unwrap_or_else(|error| {
            log::warn!("Scanning plugin manifests failed: {error}");
            Vec::new()
        })
        .into_iter()
        .flat_map(|manifest| manifest.tabs)
        .collect()
}
