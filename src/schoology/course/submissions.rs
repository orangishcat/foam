use log::info;

use crate::{
    config::config,
    schoology::{
        RequestResult, api_get_with_query,
        types::submission::{SubmissionsQuery, SubmissionsResponse},
    },
    types::{course::Course, folder::Folder, material::Material, submission::Submission},
};

// The user-specific view returns revisions, rather than the assignment-wide
// list's default of only the latest revision for each user.
// https://developers.schoology.com/api-documentation/rest-api-v1/submissions/
/// Populate the configured user's submission revisions throughout each course.
pub fn scrape_submissions(courses: &mut [Course]) -> RequestResult<()> {
    let user_id = config().user_id.to_string();
    for course in courses {
        set_submissions(&mut course.materials, &mut |assignment_id| {
            let url = format!(
                "https://api.schoology.com/v1/sections/{}/submissions/{assignment_id}/{user_id}",
                course.course_id
            );
            info!("scraping Schoology submissions: {url}");
            let response: SubmissionsResponse = api_get_with_query(
                &url,
                &SubmissionsQuery {
                    with_attachments: true,
                },
            )?;
            Ok(response
                .revision
                .into_iter()
                .filter(|revision| revision.uid.0 == user_id)
                .map(Into::into)
                .collect())
        })?;
    }
    Ok(())
}

fn set_submissions(
    folder: &mut Folder,
    fetch: &mut impl FnMut(&str) -> RequestResult<Vec<Submission>>,
) -> RequestResult<()> {
    for material in &mut folder.materials {
        match material {
            Material::Folder(folder) => set_submissions(folder, fetch)?,
            // Keep historical submissions even if submissions are now disabled.
            Material::Assignment(assignment) => assignment.submissions = fetch(&assignment.id)?,
            _ => {}
        }
    }
    Ok(())
}
