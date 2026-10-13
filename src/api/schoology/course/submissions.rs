use crate::api::schoology::account::{ResetProgress, SchoologyAccountConfig};
use crate::ui;

const SUBMISSION_PROGRESS_PRIORITY: u8 = 2;
const SUBMISSION_PROGRESS_COLOR: slint::Color = slint::Color::from_rgb_u8(180, 138, 255);
use crate::{
    api::schoology::{
        RequestResult,
        types::submission::{SubmissionsQuery, SubmissionsResponse},
    },
    types::assignment::Assignment,
};

// The user-specific view returns revisions, rather than the assignment-wide
// list's default of only the latest revision for each user.
// https://developers.schoology.com/api-documentation/rest-api-v1/submissions/

impl SchoologyAccountConfig {
    /// Populate the configured user's submission revisions for the supplied assignments.
    pub fn scrape_submissions<'a>(
        &self,
        assignments: impl IntoIterator<Item = &'a mut Assignment>,
    ) -> RequestResult<()> {
        let _reset = ResetProgress(SUBMISSION_PROGRESS_PRIORITY, SUBMISSION_PROGRESS_COLOR);
        let user_id = self.user_id.to_string();
        let assignments: Vec<_> = assignments.into_iter().collect();
        let total = assignments.len();
        if total > 0 {
            ui::publish_progress(
                1.0,
                SUBMISSION_PROGRESS_PRIORITY,
                &SUBMISSION_PROGRESS_COLOR,
            );
        }
        for (index, assignment) in assignments.into_iter().enumerate() {
            crate::thread_manager::check_cancelled()?;
            let assignment_id = &assignment.id;
            let url = format!(
                "https://api.schoology.com/v1/sections/{}/submissions/{assignment_id}/{user_id}",
                assignment.course_id
            );
            log::debug!("scraping Schoology submissions: {url}");
            let response: SubmissionsResponse = self.api_get_with_query(
                &url,
                &SubmissionsQuery {
                    with_attachments: true,
                },
            )?;
            log::debug!("submission count: {}", response.revision.len());
            assignment.submissions = response
                .revision
                .into_iter()
                .filter(|revision| revision.uid.0 == user_id)
                .map(Into::into)
                .collect();
            ui::publish_progress(
                (index + 1) as f32 / total as f32,
                SUBMISSION_PROGRESS_PRIORITY,
                &SUBMISSION_PROGRESS_COLOR,
            );
        }
        Ok(())
    }
}
