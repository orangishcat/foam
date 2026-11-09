use crate::api::schoology::account::{ResetProgress, SchoologyAccountConfig};
use crate::api::schoology::{
    RequestResult,
    types::submission::{SubmissionsQuery, SubmissionsResponse},
};

use crate::api::schoology::plugin_data::{self, Submission};

const SUBMISSION_PROGRESS_PRIORITY: u8 = 2;
const SUBMISSION_PROGRESS_COLOR: slint::Color = slint::Color::from_rgb_u8(180, 138, 255);

// The user-specific view returns revisions, rather than the assignment-wide
// list's default of only the latest revision for each user.
// https://developers.schoology.com/api-documentation/rest-api-v1/submissions/

impl SchoologyAccountConfig {
    /// Populate the configured user's submission revisions for the supplied tasks
    /// (folder_id, task id) pairs into the plugin's own table.
    pub fn scrape_submissions(
        &self,
        pairs: impl IntoIterator<Item = (String, String)>,
    ) -> RequestResult<()> {
        let _reset = ResetProgress(SUBMISSION_PROGRESS_PRIORITY, SUBMISSION_PROGRESS_COLOR);
        let user_id = self.user_id.to_string();
        let pairs: Vec<(String, String)> = pairs.into_iter().collect();
        let total = pairs.len();
        if total > 0 {
            foam::ui::publish_progress(
                1.0,
                SUBMISSION_PROGRESS_PRIORITY,
                &SUBMISSION_PROGRESS_COLOR,
            );
        }
        for (index, (folder_id, assignment_id)) in pairs.into_iter().enumerate() {
            foam::thread_manager::check_cancelled()?;
            let url = format!(
                "https://api.schoology.com/v1/sections/{folder_id}/submissions/{assignment_id}/{user_id}",
            );
            log::debug!("scraping Schoology submissions: {url}");
            let response: SubmissionsResponse = self.api_get_with_query(
                &url,
                &SubmissionsQuery {
                    with_attachments: true,
                },
            )?;
            log::debug!("submission count: {}", response.revision.len());
            let submissions: Vec<Submission> = response
                .revision
                .into_iter()
                .filter(|revision| revision.uid.0 == user_id)
                .map(Into::into)
                .collect();
            plugin_data::update_submissions(&folder_id, &assignment_id, &submissions)?;
            if let Some(row) = plugin_data::assignment(&folder_id, &assignment_id)? {
                plugin_data::publish_task_detail(&row);
            }
            foam::ui::publish_progress(
                (index + 1) as f32 / total as f32,
                SUBMISSION_PROGRESS_PRIORITY,
                &SUBMISSION_PROGRESS_COLOR,
            );
        }
        Ok(())
    }
}

impl From<crate::api::schoology::types::submission::Revision> for Submission {
    fn from(revision: crate::api::schoology::types::submission::Revision) -> Self {
        Self {
            id: revision.revision_id.0,
            user_id: revision.uid.0,
            created: chrono::DateTime::from_timestamp(revision.created.0, 0).unwrap_or_default(),
            late: revision.late.0 != 0,
            draft: revision.draft.0 != 0,
            body: revision.body,
            attachments: revision.attachments.into(),
        }
    }
}

