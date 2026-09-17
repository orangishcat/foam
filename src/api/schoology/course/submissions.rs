
use crate::{
    api::schoology::{
        RequestResult, api_get_with_query,
        types::submission::{SubmissionsQuery, SubmissionsResponse},
    },
    config::config,
    types::assignment::Assignment,
};

// The user-specific view returns revisions, rather than the assignment-wide
// list's default of only the latest revision for each user.
// https://developers.schoology.com/api-documentation/rest-api-v1/submissions/
/// Populate the configured user's submission revisions for the supplied assignments.
pub fn scrape_submissions<'a>(
    assignments: impl IntoIterator<Item = &'a mut Assignment>,
) -> RequestResult<()> {
    let user_id = config().user_id.to_string();
    for assignment in assignments {
        let assignment_id = &assignment.id;
        let url = format!(
            "https://api.schoology.com/v1/sections/{}/submissions/{assignment_id}/{user_id}",
            assignment.course_id
        );
        log::debug!("scraping Schoology submissions: {url}");
        let response: SubmissionsResponse = api_get_with_query(
            &url,
            &SubmissionsQuery {
                with_attachments: true,
            },
        )?;
        // Keep historical submissions even if submissions are now disabled.
        assignment.submissions = response
            .revision
            .into_iter()
            .filter(|revision| revision.uid.0 == user_id)
            .map(Into::into)
            .collect();
    }
    Ok(())
}
