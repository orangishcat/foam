//! The Schoology plugin's own storage (`plugin_schoology_assignments`) and
//! its task-detail publishing. All access goes through foam's core helpers
//! with the plugin authorizer scope active, so CREATE/INSERT on this
//! `plugin_schoology_*`-prefixed table follows the same policy the wasm
//! boundary enforces for out-of-process plugins.
use anyhow;
use serde::{Deserialize, Serialize};

use foam::types::attachment::Attachments;
use foam::{database, plugin::host};

pub const PLUGIN_ID: &str = "schoology";

pub const CREATE_ASSIGNMENTS_TABLE: &str = "CREATE TABLE IF NOT EXISTS plugin_schoology_assignments (
    folder_id TEXT NOT NULL,
    id TEXT NOT NULL,
    manual_mark INTEGER,
    max_points REAL,
    score REAL,
    letter_grade TEXT,
    allow_submissions INTEGER,
    submissions TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (folder_id, id)
)";

pub const CREATE_MATERIALS_TABLE: &str = "CREATE TABLE IF NOT EXISTS plugin_schoology_materials (
    kind TEXT NOT NULL,
    material_id TEXT NOT NULL,
    id TEXT,
    title TEXT,
    course_title TEXT,
    course_url TEXT,
    course_code TEXT,
    section_title TEXT,
    section_code TEXT,
    active INTEGER,
    profile_url TEXT,
    logo_img_src TEXT,
    links TEXT,
    location TEXT,
    meeting_days TEXT,
    start_time TEXT,
    end_time TEXT,
    weight TEXT,
    description TEXT,
    course_fid INTEGER,
    available INTEGER,
    published INTEGER,
    completed INTEGER,
    display_inline INTEGER,
    attachments TEXT,
    grading_period INTEGER,
    grading_scale INTEGER,
    auto_publish_grades INTEGER,
    body TEXT,
    collected_only INTEGER,
    count_in_grade INTEGER,
    due TEXT,
    max_points REAL,
    url TEXT,
    PRIMARY KEY (kind, material_id)
)";

/// Bumped when the scraped layout changes; forces a full re-scrape on the
/// next fast sync so old-shaped rows are replaced without a migration.
pub const SCRAPE_SCHEMA_VERSION: &str = "sch_materials_v7";
const SCRAPE_PROGRESS_KEY: &str = "schoology_scrape_progress";

/// Every top-level Schoology course/resource payload field has a dedicated
/// column. Structured values are JSON-encoded in that field's own column,
/// never in a catch-all blob.
const MATERIALIZED_COLUMNS: &[&str] = &[
    "id", "title", "course_title", "course_url", "course_code", "section_title",
    "section_code", "active", "profile_url", "logo_img_src", "links", "location",
    "meeting_days", "start_time", "end_time", "weight", "description", "course_fid",
    "available", "published", "completed", "display_inline", "attachments",
    "grading_period", "grading_scale", "auto_publish_grades", "body", "collected_only",
    "count_in_grade", "due", "max_points", "url",
];

/// SQL assignments for each material field on conflict.
fn materialized_column_sql() -> String {
    ["kind", "material_id"]
        .into_iter()
        .chain(MATERIALIZED_COLUMNS.iter().copied())
        .map(|name| format!("{name} = excluded.{name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One submission revision, scoped to its containing assignment and user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Submission {
    pub id: String,
    pub user_id: String,
    pub created: chrono::DateTime<chrono::Utc>,
    pub late: bool,
    pub draft: bool,
    pub body: String,
    pub attachments: Attachments,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginAssignment {
    pub folder_id: String,
    pub id: String,
    pub manual_mark: Option<bool>,
    pub max_points: Option<f64>,
    pub score: Option<f64>,
    pub letter_grade: Option<String>,
    pub allow_submissions: Option<bool>,
    #[serde(deserialize_with = "submissions_from_json")]
    pub submissions: Vec<Submission>,
}

// The submissions column is stored as a JSON string so the manifest-backed
// declarative search mapping can read it like other plugin columns.
fn submissions_from_json<'de, D>(deserializer: D) -> Result<Vec<Submission>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: String = Deserialize::deserialize(deserializer)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// Creates the plugin's own tables on the authorizer-scoped connection.
/// The materials table replaces an older 3-column shape (kind, material_id,
/// data); pre-v2 rows are dropped by the scrape-version bump, so a stale
/// table is recreated before any new-shape write can fail.
pub fn ensure_table() -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let stale = database::from_sql_map(
        "SELECT 1 FROM sqlite_master
         WHERE type = 'table' AND name = 'plugin_schoology_materials'
           AND (sql NOT LIKE '%auto_publish_grades%' OR sql LIKE '%data TEXT%')"
            .to_owned(),
        &[],
        |row| Ok(row.get::<_, i64>(0)?),
    )?
    .into_iter()
    .any(|exists| exists != 0);
    if stale {
        database::execute(
            "DROP TABLE plugin_schoology_materials".to_owned(),
            &[],
        )?;
    }
    database::execute(CREATE_ASSIGNMENTS_TABLE.to_owned(), &[])?;
    database::execute(CREATE_MATERIALS_TABLE.to_owned(), &[])?;
    Ok(())
}

/// Maps each scraped Schoology payload field onto its dedicated materials
/// column. Unknown fields fail loudly instead of being hidden in a JSON blob.
pub fn store_material(kind: &str, material_id: &str, data: &serde_json::Value) -> anyhow::Result<()> {
    use rusqlite::types::Value;

    let mut payload = data.as_object().cloned().unwrap_or_default();
    if kind == "course" {
        let title = payload.get("course_title").cloned().unwrap_or(serde_json::Value::Null);
        payload.insert("title".to_owned(), title);
        let logo = payload.get("profile_url").cloned().unwrap_or(serde_json::Value::Null);
        payload.insert("logo_img_src".to_owned(), logo);
        let course_url = payload
            .get("links")
            .and_then(|links| links.get("self"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        payload.insert("course_url".to_owned(), course_url);
    }

    let mut row_values = Vec::with_capacity(MATERIALIZED_COLUMNS.len() + 2);
    for column in MATERIALIZED_COLUMNS {
        let raw = payload.remove(*column).unwrap_or(serde_json::Value::Null);
        let value = match raw {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(state) => Value::Integer(i64::from(state)),
            serde_json::Value::Number(number) => number
                .as_i64()
                .map(Value::Integer)
                .or_else(|| number.as_f64().map(Value::Real))
                .unwrap_or(Value::Null),
            serde_json::Value::String(text) => Value::Text(text),
            value @ (serde_json::Value::Array(_) | serde_json::Value::Object(_)) => {
                Value::Text(value.to_string())
            }
        };
        row_values.push(value);
    }
    if !payload.is_empty() {
        anyhow::bail!(
            "unmapped Schoology material fields for kind {kind}: {}",
            payload.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    row_values.insert(0, Value::Text(material_id.to_owned()));
    row_values.insert(0, Value::Text(kind.to_owned()));

    let mut columns = String::from("kind, material_id, ");
    columns.push_str(
        &MATERIALIZED_COLUMNS
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .join(", "),
    );
    let placeholders = std::iter::repeat("?")
        .take(columns.split(", ").count())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO plugin_schoology_materials ({columns}) VALUES ({placeholders})
         ON CONFLICT(kind, material_id) DO UPDATE SET {}",
        materialized_column_sql()
    );
    let parameters = row_values
        .iter()
        .map(|value| value as &dyn rusqlite::ToSql)
        .collect::<Vec<&dyn rusqlite::ToSql>>();
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    database::execute(sql, &parameters)?;
    Ok(())
}

/// Lays down the scrape-version marker, invalidating a full scrape marked by
/// an older layout so the next fast sync re-scrapes from scratch.
pub fn ensure_scrape_version() -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    ensure_table()?;
    let current = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = 'scrape_version'".to_owned(),
        &[],
        |row| Ok(row.get::<_, String>(0)?),
    )?
    .into_iter()
    .next()
    .unwrap_or_default();
    if current == SCRAPE_SCHEMA_VERSION {
        return Ok(());
    }
    database::execute("DELETE FROM sync_state WHERE key = 'folders_loaded'".to_owned(), &[])?;
    database::execute(
        "INSERT INTO sync_state (key, data) VALUES (?1, '[]')
         ON CONFLICT(key) DO UPDATE SET data = '[]'"
            .to_owned(),
        &[&SCRAPE_PROGRESS_KEY],
    )?;
    database::execute(
        "INSERT INTO sync_state (key, data) VALUES ('scrape_version', ?)
         ON CONFLICT(key) DO UPDATE SET data = excluded.data"
            .to_owned(),
        &[&SCRAPE_SCHEMA_VERSION.to_string()],
    )?;
    log::info!(
        "Scrape layout changed ({}), old folders_loaded cleared for a fresh scrape",
        SCRAPE_SCHEMA_VERSION
    );
    Ok(())
}

/// Explicit resync: clears the sync-state markers that gate full fetches so
/// the next fast sync re-scrapes every folder/resource/task and the slow sync
/// rechecks submissions. The scrape layout version is kept (its bump flow
/// stays in `ensure_scrape_version`). No Schoology-side notification or
/// submission markers beyond `folders_loaded` exist in `sync_state`.
pub fn prepare_resync() -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    ensure_table()?;
    let progress_exists = database::from_sql_map(
        "SELECT 1 FROM sync_state WHERE key = ?".to_owned(),
        &[&SCRAPE_PROGRESS_KEY],
        |row| Ok(row.get::<_, i64>(0)?),
    )?
    .first()
    .is_some();
    if !progress_exists {
        database::execute("DELETE FROM sync_state WHERE key = 'folders_loaded'".to_owned(), &[])?;
        database::execute(
            "INSERT INTO sync_state (key, data) VALUES (?1, '[]')".to_owned(),
            &[&SCRAPE_PROGRESS_KEY],
        )?;
        log::info!("Resync requested: prepared resumable full scrape");
    } else {
        log::info!("Resuming incomplete Schoology full scrape");
    }
    Ok(())
}

pub fn completed_scrape_folders() -> anyhow::Result<std::collections::HashSet<String>> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let rows = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = ?".to_owned(),
        &[&SCRAPE_PROGRESS_KEY],
        |row| Ok(row.get::<_, String>(0)?),
    )?;
    Ok(rows
        .first()
        .map(|data| serde_json::from_str::<Vec<String>>(data).unwrap_or_default())
        .unwrap_or_default()
        .into_iter()
        .collect())
}

pub fn mark_scrape_folder_completed(folder_id: &str) -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let mut folders = completed_scrape_folders()?;
    folders.insert(folder_id.to_owned());
    database::execute(
        "INSERT INTO sync_state (key, data) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET data = excluded.data"
            .to_owned(),
        &[&SCRAPE_PROGRESS_KEY, &serde_json::to_string(&folders)?],
    )?;
    Ok(())
}

pub fn finish_scrape() -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    database::execute(
        "DELETE FROM sync_state WHERE key = ?".to_owned(),
        &[&SCRAPE_PROGRESS_KEY],
    )?;
    Ok(())
}

/// The task-view-era grade text (`"12 / 15 (A)"`), shared with the grades
/// notification subtitle.
pub fn grade_text(assignment: &PluginAssignment) -> String {
    match (assignment.score, assignment.letter_grade.as_deref()) {
        (Some(score), Some(letter)) => {
            format!("{score} / {} ({letter})", assignment.max_points.unwrap_or_default())
        }
        (Some(score), None) => format!("{score} / {}", assignment.max_points.unwrap_or_default()),
        (None, Some(letter)) => letter.to_owned(),
        (None, None) => "-".to_owned(),
    }
}

pub fn upsert_grade(
    folder_id: &str,
    id: &str,
    score: Option<f64>,
    letter_grade: Option<String>,
) -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    database::execute(
        "INSERT INTO plugin_schoology_assignments (folder_id, id, score, letter_grade, submissions) VALUES (?, ?, ?, ?, '[]')
         ON CONFLICT(folder_id, id) DO UPDATE SET score = excluded.score, letter_grade = excluded.letter_grade"
            .to_owned(),
        &[
            &folder_id.to_string(),
            &id.to_string(),
            &score,
            &letter_grade,
        ],
    )?;
    Ok(())
}

pub fn update_submissions(folder_id: &str, id: &str, submissions: &[Submission]) -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    database::execute(
        "UPDATE plugin_schoology_assignments SET submissions = ? WHERE folder_id = ? AND id = ?"
            .to_owned(),
        &[&serde_json::to_string(submissions)?, &folder_id.to_string(), &id.to_string()],
    )?;
    Ok(())
}

pub fn assignment(folder_id: &str, id: &str) -> anyhow::Result<Option<PluginAssignment>> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let rows = database::from_sql::<PluginAssignment>(
        "SELECT a.folder_id, a.id, a.manual_mark, a.max_points, a.score, a.letter_grade, a.allow_submissions, a.submissions
         FROM plugin_schoology_assignments a WHERE a.folder_id = ? AND a.id = ?"
            .to_owned(),
        &[&folder_id.to_string(), &id.to_string()],
    )?;
    Ok(rows.into_iter().next())
}

pub fn assignment_folder(id: &str) -> anyhow::Result<Option<String>> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let folders = database::from_sql_map(
        "SELECT DISTINCT folder_id FROM plugin_schoology_assignments WHERE id = ?".to_owned(),
        &[&id.to_owned()],
        |row| Ok(row.get::<_, String>(0)?),
    )?;
    Ok((folders.len() == 1).then(|| folders[0].clone()))
}

pub fn course_folder_by_title(title: &str) -> anyhow::Result<Option<String>> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    let folders = database::from_sql_map(
        "SELECT material_id FROM plugin_schoology_materials
         WHERE kind = 'course' AND course_title = ?".to_owned(),
        &[&title.to_owned()],
        |row| Ok(row.get::<_, String>(0)?),
    )?;
    Ok((folders.len() == 1).then(|| folders[0].clone()))
}

/// Publishes the score/letter/submission detail for one task through the
/// `task_detail_put` host path, so the generic task view renders it without
/// core tables carrying grade-specific columns.
pub fn publish_task_detail(assignment: &PluginAssignment) {
    let detail = task_detail_json(assignment);
    host::store_task_detail(PLUGIN_ID, &assignment.id, &detail.to_string());
}

/// Detail JSON shape consumed by `state/task_view.rs`:
/// `{render: "table", fields: [{label, value}, …]}` with multi-row values
/// using `|` as the cell separator.
fn task_detail_json(assignment: &PluginAssignment) -> serde_json::Value {
    let score = grade_text(assignment);    let mut fields = vec![
        serde_json::json!({"label": "Score", "value": score}),
        serde_json::json!({
            "label": "Allows submissions",
            "value": match assignment.allow_submissions {
                Some(true) => "yes",
                Some(false) => "no",
                None => "-",
            }
        }),
    ];
    if let Some(letter) = assignment.letter_grade.as_deref().filter(|l| !l.is_empty()) {
        fields.push(serde_json::json!({"label": "Letter grade", "value": letter}));
    }
    if !assignment.submissions.is_empty() {
        let rows = submission_table_rows(&assignment.submissions);
        fields.push(serde_json::json!({
            "label": "Submissions",
            "value": format!("revision|status|date\n{rows}")
        }));
    }
    serde_json::json!({"render": "table", "fields": fields})
}

fn submission_table_rows(submissions: &[Submission]) -> String {
    submissions
        .iter()
        .map(|submission| {
            let status = if submission.draft {
                "draft"
            } else if submission.late {
                "late"
            } else {
                "submitted"
            };
            let created = submission
                .created
                .format("%b %-d, %Y %-I:%M %p");
            format!("{}|{}|{}", submission.id, status, created)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub type SubmissionScrape = (
    String, // folder_id
    String, // task id
    Vec<Submission>,
);
