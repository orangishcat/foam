//! Plugin manifest handling.
//!
//! The config file IS the manifest: `<data_dir>/plugins/[ID].json` plus a
//! sibling `[ID].wasm`. A plugin is installed iff both of those files exist.

use std::path::{Path, PathBuf};

use anyhow;
use serde::{Deserialize, Serialize};

pub struct PluginManifest {
    pub id: String,
    pub enabled: bool,
    pub version: String,
    /// Arbitrary JSON static config passed to the wasm plugin's extism config.
    pub config: serde_json::Value,
    /// Optional declarative search mapping (Phase 3 seam): which columns of
    /// which plugin-owned table flow into `search_index_item`.
    pub search: Option<SearchMapping>,
    /// Optional pinned sidebar tab descriptors contributed by the plugin.
    pub tabs: Vec<PinnedTab>,
    /// Accounts the plugin provides, for the accounts registry/preferences list.
    pub accounts: Vec<PluginAccount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SearchMapping {
    pub table: String,
    pub item_type: String,
    /// Row-id column. Defaults to `id`.
    #[serde(default = "default_id_field")]
    pub id_field: String,
    /// Title column; falls back to the id when empty. Defaults to `title`.
    #[serde(default = "default_title_field")]
    pub title_field: String,
    /// Content columns; dotted names like `data.body` read JSON keys.
    #[serde(default, alias = "content_field")]
    pub content_fields: Vec<String>,
}

impl Default for SearchMapping {
    fn default() -> Self {
        Self {
            table: String::new(),
            item_type: String::new(),
            id_field: default_id_field(),
            title_field: default_title_field(),
            content_fields: Vec::new(),
        }
    }
}

fn default_id_field() -> String {
    "id".to_owned()
}

fn default_title_field() -> String {
    "title".to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PinnedTab {
    pub id: String,
    pub title: String,
    /// Optional image path/URL for the sidebar icon.
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PluginAccount {
    pub id: String,
    pub label: String,
}

/// Raw on-disk manifest shape; mirrored onto [`PluginManifest`] for access.
#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
struct ManifestFile {
    id: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    version: String,
    config: serde_json::Value,
    search: Option<SearchMapping>,
    tabs: Vec<PinnedTab>,
    accounts: Vec<PluginAccount>,
}

fn default_enabled() -> bool {
    true
}

impl ManifestFile {
    fn valid(&self) -> anyhow::Result<()> {
        if self.id.is_empty() {
            anyhow::bail!("manifest is missing an id");
        }
        if self
            .id
            .contains(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        {
            anyhow::bail!(
                "plugin id may only contain alphanumerics and '-' \\
                 (no '_', it holds the prefix boundary of plugin tables)"
            );
        }
        Ok(())
    }
}

pub fn plugins_dir() -> PathBuf {
    crate::config::config()
        .data_dir()
        .join("plugins")
        .to_path_buf()
}

pub fn manifest_path(id: &str) -> PathBuf {
    plugins_dir().join(format!("{id}.json"))
}

fn wasm_path(id: &str) -> PathBuf {
    plugins_dir().join(format!("{id}.wasm"))
}

/// Wrangles the manifest into shape inferred from the id and paths.
fn from_file(path: &Path) -> anyhow::Result<Option<PluginManifest>> {
    let id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid plugin manifest filename {path:?}"))?
        .to_owned();
    // Native account configs can share this directory but are not Extism
    // manifests. A JSON file is a plugin manifest only when its sibling wasm
    // module is installed; skip it before deserializing its unrelated schema.
    if !wasm_path(&id).exists() {
        return Ok(None);
    }
    let mut manifest: ManifestFile = crate::filesystem::read_json(path)?;
    // the filename, not the json contents, owns the plugin id
    manifest.id.clone_from(&id);
    manifest.valid()?;
    if !manifest.enabled {
        return Ok(None);
    }
    Ok(Some(PluginManifest {
        id: manifest.id,
        enabled: true,
        version: manifest.version,
        config: manifest.config,
        search: manifest.search,
        tabs: manifest.tabs,
        accounts: manifest.accounts,
    }))
}

/// All installed-and-enabled plugin manifests, in sorted id order.
pub fn scan() -> anyhow::Result<Vec<PluginManifest>> {
    let mut result = Vec::new();
    let dir = plugins_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            match from_file(&path) {
                Ok(Some(manifest)) => result.push(manifest),
                Ok(None) => {}
                Err(error) => log::warn!("skipping plugin manifest {path:?}: {error}"),
            }
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

/// Persists the enabled flag (used by the accounts settings tab; dead_code
/// until the settings toggle is wired, currently the tab is read-only).
#[allow(dead_code)]
pub fn set_enabled(plugin_id: &str, enabled: bool) -> anyhow::Result<()> {
    let path = manifest_path(plugin_id);
    let mut manifest: ManifestFile = crate::filesystem::read_json(&path)?;
    manifest.enabled = enabled;
    crate::filesystem::write_json(&path, &manifest)
        .inspect_err(|e| log::warn!("failed to write plugin manifest {path:?}: {e}"))?;
    Ok(())
}
