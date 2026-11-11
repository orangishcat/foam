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
fn attachment_roundtrip_keeps_download_path() {
    // Real wire shape from the tasks.attachments column: nested
    // files.file with a download_path used to open the file.
    let raw = r#"{"files":{"file":[{"id":"3509411628","attachment_type":"file","title":"SV_AP_Chinese_Language_Culture_Xu_MAN_1200_26_27.pdf","filename":"SV_AP_Chinese_Language_Culture_Xu_MAN_1200_26_27.pdf","filesize":334158,"md5_checksum":"40cb19b95e0f6f481ed1532979e8cdf3","timestamp":"2026-08-09T22:32:06Z","filemime":"application/pdf","download_path":"https://api.schoology.com/v1/attachment/3509411628/source/40cb19b95e0f6f481ed1532979e8cdf3.pdf","extension":"pdf"}]}}"#;
    let parsed: foam::types::attachment::Attachments = serde_json::from_str(raw).unwrap();
    assert!(parsed.files.file[0].download_path.ends_with(".pdf"), "{parsed:?}");
    // the serialized form round-trips into the same value
    let round_tripped: foam::types::attachment::Attachments =
        serde_json::from_str(&serde_json::to_string(&parsed).unwrap()).unwrap();
    assert!(
        round_tripped.files.file[0].download_path == parsed.files.file[0].download_path,
        "{round_tripped:?}"
    );
}

#[test]
fn embedded_video_url_becomes_generic_document_url() {
    let mut attachments = crate::api::schoology::course::materials::types::Attachments::default();
    attachments.extra.insert(
        "videos".into(),
        serde_json::json!({"video": [{"url": "https://video.example/watch/42"}]}),
    );
    let document = crate::api::schoology::course::materials::document::Document {
        id: crate::api::types::LooseString("42".into()),
        title: "Video".into(),
        attachments,
        ..Default::default()
    };
    let generic: foam::types::document::Document = document.into();
    assert_eq!(generic.url, "https://video.example/watch/42");
}

#[test]
fn converted_attachment_uses_pdf_preview_url() {
    let wire: crate::api::schoology::course::materials::types::FileAttachment =
        serde_json::from_str(
            r#"{
                "id": 7,
                "filename": "worksheet.docx",
                "download_path": "https://schoology.example/original.docx",
                "extension": "docx",
                "converted_download_path": "https://schoology.example/preview.pdf",
                "converted_extension": "pdf",
                "converted_filesize": 123
            }"#,
        )
        .unwrap();
    let generic: foam::types::attachment::FileAttachment = wire.into();
    assert_eq!(generic.download_path, "https://schoology.example/original.docx");
    assert_eq!(generic.preview_url, "https://schoology.example/preview.pdf");
    assert_eq!(generic.preview_extension, "pdf");
    assert_eq!(generic.preview_filesize, 123);
}

#[test]
fn submission_revision_without_body_is_still_usable() {
    let revision: crate::api::schoology::types::submission::Revision = serde_json::from_value(
        serde_json::json!({"revision_id": "7", "uid": "user", "created": 1}),
    )
    .unwrap();
    assert!(revision.body.is_empty());
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
