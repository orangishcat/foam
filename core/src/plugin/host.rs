//! Host functions exposed to wasm plugins and the SQLite authorizer.
//!
//! ## Threading
//!
//! All wasm plugin execution is serialized behind [`REGISTRY`], see
//! `plugin/mod.rs`. Only one plugin call runs at any instant, so the single
//! [`ACTIVE_PLUGIN`] cell is un-ambiguous: it is set for the duration of a
//! call and no second call can interleave. The authorizer consults it while
//! SQL is prepared on the shared connection; since plugin SQL runs while
//! that call owns the plugin runtime lock, the authorizer's view of "which
//! plugin is running" cannot be swapped mid-flight.
//!
//! The shared `Connection` behind `database::connection()` is also guarded by
//! its own Mutex, so a host function may lock the DB connection without risk
//! of concurrent native access.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow;
use extism::{CurrentPlugin, Error, UserData, Val};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::types::Value as SqlValue;
use serde_json::Value as JsonValue;

use super::manifest::SearchMapping;

/// Tables foam core owns and plugins may freely write.
const CORE_TABLES: [&str; 8] = [
    "folders",
    "resources",
    "tasks",
    "notifications",
    "search_mapping",
    "search_index",
    "sync_state",
    "attachments",
];

/// The `plugin_<id>_` table prefix a plugin owns, or `None` when the app
/// itself (not a plugin) is executing SQL.
static ACTIVE_PLUGIN: Mutex<Option<String>> = Mutex::new(None);

// dead_code outside tests until the wasm plugin path lands (Phase 4).
#[allow(dead_code)]
pub fn set_active_plugin(plugin_id: Option<String>) {
    *ACTIVE_PLUGIN
        .lock()
        .expect("plugin authorizer state is poisoned") = plugin_id;
}

pub fn active_plugin() -> Option<String> {
    ACTIVE_PLUGIN
        .lock()
        .expect("plugin authorizer state is poisoned")
        .clone()
}

/// The `plugin_<id>_`-prefixed table's owning plugin id. Plugin ids never
/// contain `_` (see manifest validation), so the first `_` after the `plugin_`
/// delimiter is the id boundary.
fn table_owning_prefix(table: &str) -> Option<String> {
    let rest = table.strip_prefix("plugin_")?;
    Some(match rest.split_once("_") {
        Some((id, _suffix)) => id.to_owned(),
        None => rest.to_owned(),
    })
}

/// True when `table` is an FTS5/other virtual table shadow table of a core table,
/// e.g. `search_index_docsize` belongs to the `search_index` FTS5 table. Writes
/// triggered on core tables fan out into these internals transparently, so
/// plugins writing core tables must be allowed to touch them too.
fn is_core_shadow_table(table: &str) -> bool {
    const SHADOW_SUFFIXES: [&str; 4] = ["_data", "_docsize", "_idx", "_config"];
    const SHADOW_CONTENTED_SUFFIXES: [&str; 2] = ["_content", "_contentname"];
    CORE_TABLES.iter().any(|base| {
        table
            .strip_prefix(base)
            .is_some_and(|rest| SHADOW_SUFFIXES.contains(&rest) || SHADOW_CONTENTED_SUFFIXES.contains(&rest))
    })
}

pub fn authorize(ctx: AuthContext<'_>) -> Authorization {
    authorize_action(ctx.action)
}

pub(crate) fn authorize_action(action: AuthAction<'_>) -> Authorization {
    let Some(active) = active_plugin() else {
        return Authorization::Allow; // the app itself inside every door
    };
    let prefix = format!("plugin_{active}_");
    match action {
        // All reads on every table are allowed (including core + other plugins).
        AuthAction::Read { .. } | AuthAction::Select | AuthAction::Analyze { .. } => {
            Authorization::Allow
        }
        AuthAction::Transaction { .. } | AuthAction::Savepoint { .. } | AuthAction::Recursive => {
            Authorization::Allow
        }
        AuthAction::Function { .. } => Authorization::Allow,
        // Core-table writes are always allowed for plugins.
        AuthAction::Insert { table_name }
        | AuthAction::Delete { table_name }
        | AuthAction::Update {
            table_name,
            column_name: _,
        } => {
            // dropping/creating own tables makes sqlite delete bookkeeping
            // rows out of sqlite_master; SQLite itself disallows any other
            // direct write to the schema table (newer versions only via
            // writable_schema pragma)
            if table_name == "sqlite_master" || table_name == "sqlite_temp_master" {
                Authorization::Allow
            } else if CORE_TABLES.contains(&table_name) || is_core_shadow_table(table_name) {
                Authorization::Allow
            } else if table_owning_prefix(table_name) == Some(active.clone()) {
                Authorization::Allow
            } else {
                log::debug!("plugin {active} denied writing table {table_name}");
                Authorization::Deny
            }
        }
        AuthAction::CreateTable { table_name }
        | AuthAction::CreateIndex { index_name: _, table_name } => {
            if table_name.starts_with(&prefix) {
                Authorization::Allow
            } else {
                log::debug!("plugin {active} denied creating table {table_name}");
                Authorization::Deny
            }
        }
        AuthAction::DropTable { table_name }
        | AuthAction::DropIndex { index_name: _, table_name } => {
            if table_name.starts_with(&prefix) {
                Authorization::Allow
            } else {
                log::debug!("plugin {active} denied dropping table {table_name}");
                Authorization::Deny
            }
        }
        AuthAction::AlterTable { table_name, .. } => {
            if table_name.starts_with(&prefix) {
                Authorization::Allow
            } else {
                log::debug!("plugin {active} denied altering table {table_name}");
                Authorization::Deny
            }
        }
        AuthAction::DropView { .. }
        | AuthAction::DropTrigger { .. }
        | AuthAction::DropTempTrigger { .. }
        | AuthAction::DropTempView { .. } => Authorization::Deny,
        AuthAction::CreateTrigger { .. }
        | AuthAction::CreateTempTrigger { .. }
        | AuthAction::CreateView { .. }
        | AuthAction::CreateTempView { .. } => Authorization::Deny,
        AuthAction::Pragma { pragma_name, .. } => {
            // SQLite virtual-table modules (including FTS5) inspect table
            // metadata while opening a virtual table. Whitelist read-only
            // metadata PRAGMAs; SQLite passes their argument in pragma_value,
            // so checking for a missing value would incorrectly reject them.
            const READ_ONLY_PRAGMAS: [&str; 9] = [
                "table_info",
                "table_xinfo",
                "index_list",
                "index_info",
                "index_xinfo",
                "foreign_key_list",
                "compile_options",
                "collation_list",
                "data_version",
            ];
            if READ_ONLY_PRAGMAS
                .iter()
                .any(|name| pragma_name.eq_ignore_ascii_case(name))
            {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }
        AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::Reindex { .. }
        | AuthAction::CreateVtable { .. }
        | AuthAction::DropVtable { .. } => Authorization::Deny,
        AuthAction::Unknown { .. } => Authorization::Deny,
        _ => {
            log::debug!("plugin {active} denied unknown authorizer action");
            Authorization::Deny
        }
    }
}

/// Installed ONCE on the shared connection from `database::init()`.
pub fn install_authorizer(connection: &rusqlite::Connection) -> anyhow::Result<()> {
    connection.authorizer(Some(authorize))?;
    Ok(())
}

// ---
// In-memory extension data for the task view (Phase 2 skeleton). Plugins push
// detail JSON for tasks; the task view may consult `task_details` when
// rendering details instead of round-tripping a table.
// ---

/// Keyed by (plugin_id, task_id); last writer wins per key like a upsert-less put.
/// Persists detail data for a task, keyed (plugin_id, task_id); last writer
/// wins per key like a put. The task view consults `task_details` when
/// rendering details instead of round-tripping a table.
static TASK_DETAILS: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();

pub fn store_task_detail(plugin_id: &str, task_id: &str, json: &str) {
    let map = TASK_DETAILS.get_or_init(|| Mutex::new(HashMap::new()));
    map.lock()
        .expect("task details lock is poisoned")
        .insert((plugin_id.to_owned(), task_id.to_owned()), json.to_owned());
}

/// All plugin detail JSON entries for a task, as (plugin_id, json) pairs.
// dead_code until the task view consults plugin detail entries (Phase 3).
#[allow(dead_code)]
pub fn task_details(task_id: &str) -> Vec<(String, String)> {
    let Some(map) = TASK_DETAILS.get() else {
        return Vec::new();
    };
    let map = map.lock().expect("task details lock is poisoned");
    map.iter()
        .filter(|((_, id), _)| id == task_id)
        .map(|((plugin, _id), json)| (plugin.clone(), json.clone()))
        .collect()
}

// ---
// Host functions. Every scalar crosses the boundary as an extism pointer;
// `host_fn!` decodes typed values through extism-convert.
// ---

/// `plugin_db_exec(sql: String, params: serde_json::Value) -> i64`.
/// Params are a JSON array of values (json string, number, bool, null).
/// Only raw-SQL accessor; authorized by the installed authorizer.
pub(super) fn db_exec(
    _plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), Error> {
    let sql: String = _plugin.memory_get_val(&inputs[0])?;
    let params: JsonValue = _plugin.memory_get_val(&inputs[1])?;
    let count = run_db(|| -> anyhow::Result<i64> {
        let total = crate::database::connection()?
            .prepare_cached(&sql)?
            .execute(rusqlite::params_from_iter(params_from_json(&params)?))?;
        anyhow::Ok(total as i64)
    })?;
    if !outputs.is_empty() {
        let handle = _plugin.memory_new(&count)?;
        outputs[0] = _plugin.memory_to_val(handle);
    }
    Ok(())
}

/// `plugin_db_query(sql: String, params: serde_json::Value) -> serde_json::Value`
/// (rows as json objects).
pub(super) fn db_query(
    _plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), Error> {
    let sql: String = _plugin.memory_get_val(&inputs[0])?;
    let params: JsonValue = _plugin.memory_get_val(&inputs[1])?;
    let rows = run_db(|| -> anyhow::Result<JsonValue> {
        let connection = crate::database::connection()?;
        let mut statement = connection.prepare_cached(&sql)?;
        let columns: Vec<String> = statement
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut rows: Vec<JsonValue> = Vec::new();
        let mut statement = statement.query(rusqlite::params_from_iter(params_from_json(&params)?))?;
        while let Some(row) = statement.next()? {
            let mut object = serde_json::Map::new();
            for (index, column) in columns.iter().enumerate() {
                let value: SqlValue = row.get(index)?;
                object.insert(
                    column.to_string(),
                    serde_json::Value::from(match value {
                        SqlValue::Null => serde_json::Value::Null,
                        SqlValue::Integer(n) => serde_json::Value::from(n),
                        SqlValue::Real(n) => serde_json::Value::from(n),
                        SqlValue::Text(s) => serde_json::Value::from(s),
                        SqlValue::Blob(b) => serde_json::Value::from(b),
                    }),
                );
            }
            rows.push(serde_json::Value::Object(object));
        }
        Ok(serde_json::Value::Array(rows))
    })?;
    if !outputs.is_empty() {
        let handle = _plugin.memory_new(&rows)?;
        outputs[0] = _plugin.memory_to_val(handle);
    }
    Ok(())
}

/// `task_detail_put(task_id: String, json: String) -> ()`.
pub(super) fn task_detail_put(
    _plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), Error> {
    let task_id: String = _plugin.memory_get_val(&inputs[0])?;
    let json: String = _plugin.memory_get_val(&inputs[1])?;
    let plugin_id = active_plugin().unwrap_or_default();
    store_task_detail(&plugin_id, &task_id, &json);
    Ok(())
}

/// `search_index_item(item_type, item_id, folder_id, title, content) -> ()`
/// Writes `search_mapping` + `search_index` with `source = plugin:<id>` so
/// plugin rows land in search without authored SQL triggers.
pub(super) fn search_index_item(
    _plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), Error> {
    let item_type: String = _plugin.memory_get_val(&inputs[0])?;
    let item_id: String = _plugin.memory_get_val(&inputs[1])?;
    let folder_id: String = _plugin.memory_get_val(&inputs[2])?;
    let title: String = _plugin.memory_get_val(&inputs[3])?;
    let content: String = _plugin.memory_get_val(&inputs[4])?;
    let source = attribution();
    run_db(|| {
        let mut connection = crate::database::connection()?;
        let transaction = connection.transaction()?;
        index_item_on(&transaction, &item_type, &item_id, &folder_id, &title, &content, &source)?;
        Ok(transaction.commit()?)
    })?;
    Ok(())
}

/// Writes one row into `search_mapping` + its fts `search_index` entry inside
/// the caller's transaction. Shared by the wasm host fn and the declarative
/// search-mapping application.
pub(crate) fn index_item_on(
    transaction: &rusqlite::Transaction,
    item_type: &str,
    item_id: &str,
    folder_id: &str,
    title: &str,
    content: &str,
    source: &str,
) -> anyhow::Result<()> {
    transaction.execute(
        "INSERT INTO search_mapping (item_type, item_id, folder_id, source) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT DO NOTHING",
        rusqlite::params![item_type, item_id, folder_id, source],
    )?;
    transaction.execute(
        "INSERT INTO search_index (rowid, title, content, item_type, item_id)
         SELECT id, ?4, ?5, ?1, ?2
         FROM search_mapping
         WHERE item_type = ?1 AND item_id = ?2 AND folder_id = ?3 AND source = ?4
         ON CONFLICT DO NOTHING",
        rusqlite::params![item_type, item_id, folder_id, title, content, source],
    )?;
    Ok(())
}

/// Source attributed to rows the authorizer considers plugin-owned in the
/// current call context (`plugin:<id>`), or `builtin` inside the app itself.
pub fn attribution() -> String {
    active_plugin()
        .map(|plugin| format!("plugin:{plugin}"))
        .unwrap_or_else(|| "builtin".to_owned())
}

/// Test seam: the mapping field expression used by `apply_search_mapping`.
#[doc(hidden)]
pub fn mapping_field_expr_for_test(field: &str) -> String {
    field_expr(field)
}

/// SQL expression for a mapping field: `data.body` reads the JSON key `body`
/// out of column `data`; a bare name is the column itself.
fn field_expr(field: &str) -> String {
    match field.split_once('.') {
        Some((column, path)) => format!("json_extract({column}, '$.{path}')"),
        None => field.to_owned(),
    }
}

/// Applies the manifest's declarative search mapping: reads the plugin's own
/// prefixed table (via the active-plugin authorizer context) and indexes every
/// row through the same path the `search_index_item` host function uses, with
/// `source = plugin:<id>` so the entries can be wiped on reload.
///
/// Semantics:
/// - `table` must be `plugin_<id>_<anything>` (validated, not taken on trust).
/// - `id_field` (default `id`) identifies the row; rows with a NULL id are skipped.
/// - `title_field` (default `title`); falls back to the id when NULL/empty.
/// - `content_fields` (accepts `content_field` alias) each become either a
///   column or, for dotted names like `data.body`, a `json_extract` of it;
///   non-empty values are joined with newlines into the fts content.
/// - `folder_id` is empty for plugin-mapped rows (no natural folder).
/// - Re-running the mapping first wipes all `plugin:<id>` rows, so it is
///   idempotent when the plugin rewrites its tables.
pub fn apply_search_mapping(plugin_id: &str, search: &SearchMapping) -> anyhow::Result<usize> {
    let source = format!("plugin:{plugin_id}");
    if !search
        .table
        .starts_with(&format!("plugin_{plugin_id}_"))
    {
        anyhow::bail!(
            "plugin {plugin_id} search mapping table {} is not plugin_<id>_ prefixed",
            search.table
        );
    }
    set_active_plugin(Some(plugin_id.to_owned()));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> anyhow::Result<usize> {
        let mut connection = crate::database::connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM search_index WHERE rowid IN (
                SELECT id FROM search_mapping WHERE source = ?1)",
            rusqlite::params![source],
            )?;
        transaction.execute(
            "DELETE FROM search_mapping WHERE source = ?1",
            rusqlite::params![source],
        )?;
        let mut indexed = 0;
        {
            let columns: Vec<String> = std::iter::once(format!(
                "COALESCE({}, '') AS mapped_title, {} AS mapped_id",
                field_expr(&search.title_field),
                search.id_field
            ))
            .chain(search.content_fields.iter().map(|field| field_expr(field)))
            .collect();
            let mut statement = transaction.prepare(&format!(
                "SELECT {} FROM {} WHERE {} IS NOT NULL",
                columns.join(", "),
                search.table,
                search.id_field
            ))?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let title: String = row.get(0)?;
                let item_id: String = row.get(1)?;
                let mut parts: Vec<String> = Vec::new();
                for index in 2..columns.len() {
                    if let Some(value) = row.get::<_, Option<String>>(index)? {
                        if !value.trim().is_empty() {
                            parts.push(value);
                        }
                    }
                }
                let title = if title.trim().is_empty() { &item_id } else { &title };
                index_item_on(
                    &transaction,
                    &search.item_type,
                    &item_id,
                    "",
                    title,
                    &parts.join("\n"),
                    &source,
                )?;
                indexed += 1;
            }
        }
        transaction.commit()?;
        anyhow::Ok(indexed)
    }));
    set_active_plugin(None);
    outcome
        .unwrap_or_else(|payload| {
            Err(anyhow::anyhow!("apply_search_mapping panicked: {payload:?}"))
        })
        .and_then(|inner| Ok(inner))
        .map_err(|err| anyhow::anyhow!("applying plugin {plugin_id} search mapping: {err}"))
}

/// Removes every `plugin:<id>`-attributed search row (used when the mapping is
/// gone from the manifest so stale index entries do not linger).
pub fn clear_mapped_search(plugin_id: &str) -> anyhow::Result<()> {
    let source = format!("plugin:{plugin_id}");
    let mut connection = crate::database::connection()?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "DELETE FROM search_index WHERE rowid IN (
            SELECT id FROM search_mapping WHERE source = ?1)",
        rusqlite::params![source],
    )?;
    transaction.execute(
        "DELETE FROM search_mapping WHERE source = ?1",
        rusqlite::params![source],
    )?;
    Ok(transaction.commit()?)
}

/// Placeholder for Phase 4: `http_request` with the app-managed cookie jar.
pub(super) fn http_request(
    _: &mut CurrentPlugin,
    _: &[Val],
    _: &mut [Val],
    _: UserData<()>,
) -> Result<(), Error> {
    Err(anyhow::anyhow!("http_request is not implemented yet (phase 4)"))
}

/// Placeholder for Phase 4: cache a remote icon/attachment and return a path.
pub(super) fn attachment_cache_url(
    _: &mut CurrentPlugin,
    _: &[Val],
    _: &mut [Val],
    _: UserData<()>,
) -> Result<(), Error> {
    Err(anyhow::anyhow!(
        "attachment_cache_url is not implemented yet (phase 4)"
    ))
}

/// Runs a DB-touching closure, reconciling any panic so it can not unwind
/// through the extism host boundary.
fn run_db<T>(f: impl FnOnce() -> anyhow::Result<T>) -> Result<T, Error> {
    // never unwind through the extism host boundary; extism FFI forbids it
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    result
        .unwrap_or_else(|payload| {
            Err::<T, _>(Error::msg(format!(
                "host function panicked while accessing database: {payload:?}"
            )))
        })
        .map_err(|err| Error::msg(err.to_string()))
}

fn params_from_json(params: &JsonValue) -> Result<Vec<SqlValue>, Error> {
    let Some(array) = params.as_array() else {
        return Err(Error::msg("params must be a JSON array"));
    };
    Ok(array
        .iter()
        .map(|value| match value {
            JsonValue::Null => SqlValue::Null,
            JsonValue::Bool(b) => SqlValue::Integer((*b).into()),
            JsonValue::Number(n) => {
                if let Some(int) = n.as_i64() {
                    SqlValue::Integer(int)
                } else {
                    SqlValue::Real(n.as_f64().unwrap_or(0.0))
                }
            }
            JsonValue::String(s) => SqlValue::Text(s.clone()),
            JsonValue::Array(_) | JsonValue::Object(_) => {
                SqlValue::Text(value.to_string())
            }
        })
        .collect())
}

/// Host functions registered with every plugin instance.
pub fn host_functions() -> Vec<extism::Function> {
    use extism::{Function, PTR};
    vec![
        Function::new(
            "plugin_db_exec",
            [PTR, PTR],
            [PTR],
            UserData::new(()),
            db_exec,
        ),
        Function::new(
            "plugin_db_query",
            [PTR, PTR],
            [PTR],
            UserData::new(()),
            db_query,
        ),
        Function::new(
            "task_detail_put",
            [PTR, PTR],
            [],
            UserData::new(()),
            task_detail_put,
        ),
        Function::new(
            "search_index_item",
            [PTR; 5],
            [],
            UserData::new(()),
            search_index_item,
        ),
        Function::new(
            "http_request",
            [PTR; 5],
            [PTR],
            UserData::new(()),
            http_request,
        ),
        Function::new(
            "attachment_cache_url",
            [PTR],
            [PTR],
            UserData::new(()),
            attachment_cache_url,
        ),
    ]
}
