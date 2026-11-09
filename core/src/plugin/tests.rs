// Tests for the SQLite authorizer policy (plugin/host.rs), exercised directly
// on rusqlite without extism, and for the in-memory task detail registry.

use crate::plugin::host;

fn connection() -> rusqlite::Connection {
    let connection = rusqlite::Connection::open_in_memory().expect("open test db");
    connection
        .execute_batch(
            "CREATE TABLE folders (folder_id TEXT PRIMARY KEY, title TEXT);
             CREATE TABLE plugin_userone_items (id TEXT);
             CREATE TABLE plugin_other_items (id TEXT);",
        )
        .expect("create tables");
    connection
        .authorizer(Some(host::authorize))
        .expect("install authorizer");
    connection
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

/// Serialize tests that flip the process-global active-plugin switch.
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn assert_denied(connection: &rusqlite::Connection, sql: &str) {
    match connection.prepare(sql) {
        Err(error) if error.to_string().contains("not authorized") => {}
        Err(error) => panic!("unexpected error for {sql:?}: {error}"),
        Ok(_) => panic!("statement should be denied: {sql}"),
    }
}

fn assert_allowed(connection: &rusqlite::Connection, sql: &str) {
    connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("statement should be allowed: {sql} ({error})"));
}

#[test]
fn app_itself_has_full_access() {
    let _lock = lock();
    let connection = connection();
    host::set_active_plugin(None);
    assert_allowed(&connection, "DROP TABLE plugin_other_items");
    assert_allowed(&connection, "INSERT INTO folders VALUES ('1', 'ok')");
    assert_allowed(&connection, "CREATE TABLE whatever_the_app_wants (id TEXT)");
}

#[test]
fn plugin_writes_core_tables_and_own_tables() {
    let _lock = lock();
    let connection = connection();
    host::set_active_plugin(Some("userone".into()));
    assert_allowed(
        &connection,
        "INSERT INTO plugin_userone_items VALUES ('x')",
    );
    assert_allowed(&connection, "UPDATE plugin_userone_items SET id = 'x'");
    assert_allowed(&connection, "DELETE FROM plugin_userone_items");
    assert_allowed(&connection, "DROP TABLE plugin_userone_items");
    assert_allowed(&connection, "CREATE TABLE plugin_userone_notes (id TEXT)");
    assert_allowed(&connection, "INSERT INTO folders VALUES ('1', 'ok')");
    assert_allowed(&connection, "DELETE FROM folders");
    assert_allowed(&connection, "SELECT * FROM folders");
    assert_allowed(&connection, "SELECT * FROM plugin_other_items");
    host::set_active_plugin(None);
}

#[test]
fn plugin_is_denied_foreign_tables_and_db_level_actions() {
    let _lock = lock();
    let connection = connection();
    host::set_active_plugin(Some("userone".into()));
    for sql in [
        "INSERT INTO plugin_other_items VALUES ('x')",
        "UPDATE plugin_other_items SET id = 'x'",
        "DELETE FROM plugin_other_items",
        "DROP TABLE plugin_other_items",
        "CREATE TABLE other_plugin_notes (id TEXT)",
        "ALTER TABLE plugin_other_items ADD COLUMN c TEXT",
        "CREATE TABLE regular_app_table (id TEXT)",
        "DROP TABLE folders",
        "ATTACH 'x.db' AS rogue",
        "PRAGMA journal_mode = DELETE",
        "CREATE TRIGGER rogue AFTER INSERT ON folders BEGIN SELECT 1; END",
        "CREATE VIEW rogue AS SELECT 1",
    ] {
        assert_denied(&connection, sql);
    }
    host::set_active_plugin(None);
}

#[test]
fn plugin_can_read_metadata_but_cannot_set_pragmas() {
    let _lock = lock();
    let connection = connection();
    host::set_active_plugin(Some("userone".into()));
    assert_allowed(&connection, "PRAGMA table_info(folders)");
    assert_denied(&connection, "PRAGMA journal_mode = DELETE");
    host::set_active_plugin(None);
}

#[test]
fn plugin_core_writes_can_update_the_fts_virtual_table() {
    let _lock = lock();
    host::set_active_plugin(None);
    let connection = rusqlite::Connection::open_in_memory().expect("open test db");
    connection
        .execute_batch(include_str!("../sql/schema.sql"))
        .expect("create core schema");
    connection
        .authorizer(Some(host::authorize))
        .expect("install authorizer");

    host::set_active_plugin(Some("schoology".into()));
    connection
        .execute(
            "INSERT INTO folders (folder_id, title, data) VALUES ('f', 'Folder', '{}')",
            [],
        )
        .expect("plugin can insert a core folder");
    connection
        .execute(
            "INSERT INTO resources (folder_id, resource_id, type, parent_id, title, data) VALUES ('f', 't', 'task', '', 'Task', '{}')",
            [],
        )
        .expect("plugin can insert a core resource");
    connection
        .execute(
            "INSERT INTO tasks (folder_id, id, title, description, due, attachments) VALUES ('f', 't', 'Task', 'Description', NULL, '[]')",
            [],
        )
        .expect("core trigger can update FTS5 index for plugin writes");
    host::set_active_plugin(None);
}

#[test]
fn known_unknown_action_codes_default_to_deny() {
    // a plugin with a crafted odd-name CREATE handles nothing; the matcher
    // denies all non-read actions on unknown names through prefix mismatch
    let _lock = lock();
    let connection = connection();
    host::set_active_plugin(Some("userone".into()));
    assert_denied(&connection, "REINDEX folders");
    host::set_active_plugin(None);
}

#[test]
fn task_details_round_trip() {
    host::store_task_detail("schoology", "42", "{\"grades\": {}}");
    host::store_task_detail("other", "42", "{\"extra\": 7}");
    let details = host::task_details("42");
    assert_eq!(details.len(), 2);
    assert!(details.contains(&("schoology".into(), "{\"grades\": {}}".into())));
    assert!(details.contains(&("other".into(), "{\"extra\": 7}".into())));
    assert_eq!(host::task_details("43").len(), 0);
}

#[test]
fn mapping_field_expr_reads_dotted_paths_as_json() {
    assert_eq!(host::mapping_field_expr_for_test("data.body"), "json_extract(data, '$.body')");
    assert_eq!(host::mapping_field_expr_for_test("url"), "url");
}

#[test]
fn search_mapping_rejects_foreign_table() {
    let mapping = crate::plugin::manifest::SearchMapping {
        table: "plugin_other_items".into(),
        ..Default::default()
    };
    let result = host::apply_search_mapping("userone", &mapping);
    assert!(result.is_err(), "mapping another plugin's table must be denied");
    // no plugin gets left active after the validation failure
    assert_eq!(host::active_plugin(), None);
}
