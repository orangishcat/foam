//! Schoology grading feed -> the plugin's own `plugin_schoology_assignments`
//! table (Phase 0 moved the old `assignments` grade columns to plugin-owned
//! storage; grades are restored through the plugin detail path).
use std::collections::HashMap;

use crate::api::schoology::account::SchoologyAccountConfig;
use crate::api::schoology::plugin_data;
use crate::api::schoology::types::grades::GradesResponse;
use crate::api::schoology::RequestResult;

type GradeUpdate = (Option<f64>, Option<String>, String, String);

pub(crate) fn grade_updates(
    response: &GradesResponse,
) -> anyhow::Result<Vec<GradeUpdate>> {
    let mut scores = HashMap::new();
    for section in &response.section {
        // Resolve a section id through folder aliases just like the fallback
        // used everywhere else; folders are stored before grades are applied.
        let Some(folder) = foam::types::folder_info::folder(&section.section_id.0)? else {
            continue;
        };
        for grade in section.period.iter().flat_map(|period| &period.assignment) {
            let value = grade
                .grade
                .as_ref()
                .map(|grade| grade.0.trim())
                .filter(|grade| !grade.is_empty());
            let score = value
                .and_then(|grade| grade.parse::<f64>().ok())
                .filter(|score| score.is_finite());
            let letter = value.filter(|_| score.is_none()).map(str::to_owned);
            scores.insert(
                (folder.folder_id.clone(), grade.assignment_id.0.clone()),
                (score, letter),
            );
        }
    }
    Ok(scores
        .into_iter()
        .map(|((folder_id, id), (score, letter))| (score, letter, folder_id, id))
        .collect())
}

impl SchoologyAccountConfig {
    pub fn scrape_grades(&self) -> RequestResult<()> {
        self.apply_grades(&self.fetch_grades()?)?;
        Ok(())
    }

    pub(crate) fn fetch_grades(&self) -> RequestResult<GradesResponse> {
        let url = format!("https://api.schoology.com/v1/users/{}/grades", self.user_id);
        self.api_get(&url)
    }

    pub(crate) fn apply_grades(&self, response: &GradesResponse) -> anyhow::Result<()> {
        for (score, letter, folder_id, id) in grade_updates(response)? {
            plugin_data::upsert_grade(&folder_id, &id, score, letter)?;
            // Refresh the generic task detail rows with the new score state.
            if let Some(assignment) = plugin_data::assignment(&folder_id, &id)? {
                plugin_data::publish_task_detail(&assignment);
            }
        }
        Ok(())
    }

    /// The task-detail-era subtitle for one notification's resource, matching
    /// the old assignment-view grade text (`"12 / 15"` / `"A"` / `"12 / 15 (A)"`).
    pub(crate) fn grade_subtitle(&self, folder_id: &str, id: &str) -> anyhow::Result<String> {
        Ok(plugin_data::assignment(folder_id, id)?
            .map(|row| plugin_data::grade_text(&row))
            .unwrap_or_default())
    }
}
