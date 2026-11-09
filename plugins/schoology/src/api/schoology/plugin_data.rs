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

/// Creates the plugin's own table on the authorizer-scoped connection.
pub fn ensure_table() -> anyhow::Result<()> {
    let _scope = foam::plugin::scoped(PLUGIN_ID);
    database::execute(CREATE_ASSIGNMENTS_TABLE.to_owned(), &[])?;
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
