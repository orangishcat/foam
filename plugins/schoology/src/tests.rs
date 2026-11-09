//! Authorizer-policy tests for the native Schoology account: the same
//! scoping rules wasm plugins get from the host fns, exercised on an
//! in-memory connection with foam's authorizer installed.

use crate::api::schoology::plugin_data;
use foam::plugin::host;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// An in-memory connection with the authorizer installed, mirroring the
/// table shape the app owns (`folders`, `tasks`) plus one foreign plugin table.
fn connection() -> rusqlite::Connection {
    let connection = rusqlite::Connection::open_in_memory().expect("open test db");
    connection
        .execute_batch(
            "CREATE TABLE folders (folder_id TEXT PRIMARY KEY, title TEXT);
             CREATE TABLE tasks (folder_id TEXT, id TEXT, title TEXT);
             CREATE TABLE plugin_other_items (id TEXT);",
        )
        .expect("create tables");
    connection
        .authorizer(Some(host::authorize))
        .expect("install authorizer");
    connection
}

fn denied(connection: &rusqlite::Connection, sql: &str) {
    match connection.prepare(sql) {
        Err(error) if error.to_string().contains("not authorized") => {}
        Err(error) => panic!("unexpected error for {sql:?}: {error}"),
        Ok(_) => panic!("statement should be denied: {sql}"),
    }
}

#[test]
fn schoology_scope_writes_own_and_core_tables() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let _scope = foam::plugin::scoped("schoology");
    assert_eq!(host::active_plugin().as_deref(), Some("schoology"));
    let connection = connection();
    connection.execute(plugin_data::CREATE_ASSIGNMENTS_TABLE, ()).expect("create own table");
    connection
        .execute(
            "INSERT INTO plugin_schoology_assignments (folder_id, id, score, letter_grade, submissions)
             VALUES ('1', '2', 5.0, NULL, '[]')",
            (),
        )
        .expect("write own table");
    connection.execute("INSERT INTO folders VALUES ('1', 'ok')", ()).expect("core write");
    connection
        .execute("INSERT INTO tasks VALUES ('1', '2', 'hi')", ())
        .expect("core write");
    denied(&connection, "INSERT INTO plugin_other_items VALUES ('x')");
    denied(&connection, "DROP TABLE plugin_other_items");
    denied(&connection, "CREATE TABLE app_owned_table (id TEXT)");
    denied(&connection, "ATTACH 'x.db' AS rogue");
    denied(&connection, "PRAGMA journal_mode = DELETE");
    drop(connection);
    drop(_scope); // scope guard restores the no-plugin state
    assert_eq!(host::active_plugin(), None);
}

#[test]
fn task_detail_publishes_score_fields() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let assignment = plugin_data::PluginAssignment {
        folder_id: "f".into(),
        id: "42".into(),
        score: Some(9.5),
        max_points: Some(10.0),
        letter_grade: Some("A".into()),
        submissions: vec![plugin_data::Submission {
            id: "r1".into(),
            user_id: "u".into(),
            late: true,
            draft: false,
            ..Default::default()
        }],
        ..Default::default()
    };
    plugin_data::publish_task_detail(&assignment);
    let details = host::task_details("42");
    assert_eq!(details.len(), 1);
    let (plugin, json) = &details[0];
    assert_eq!(plugin, "schoology");
    assert!(json.contains("\"Score\""), "{json}");
    assert!(json.contains("9.5 / 10"), "{json}");
    assert!(json.contains("\"A\""), "{json}");
    assert!(json.contains("late"), "{json}");
    assert!(json.contains("\"table\""), "{json}");
}
