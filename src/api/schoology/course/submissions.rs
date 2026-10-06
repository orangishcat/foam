use crate::api::schoology::account::SchoologyAccountConfig;
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
        let user_id = self.user_id.to_string();
        for assignment in assignments {
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
        }
        Ok(())
    }
}
