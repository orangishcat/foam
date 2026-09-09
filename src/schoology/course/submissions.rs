use log::info;
use serde::{Deserialize, Serialize};

use crate::{
    config::config,
    schoology::{RequestResult, api_get_with_query},
    types::{
        LooseString, course::Course, folder::Folder, material::Material, submission::Submission,
    },
};

use super::materials::types::{Attachments, LooseInt};

#[derive(Serialize, oauth::Request)]
struct SubmissionsQuery {
    with_attachments: bool,
}

// The user-specific view returns revisions, rather than the assignment-wide
// list's default of only the latest revision for each user.
// https://developers.schoology.com/api-documentation/rest-api-v1/submissions/
#[derive(Deserialize)]
struct SubmissionsResponse {
    #[serde(default)]
    revision: Vec<Revision>,
}

#[derive(Deserialize)]
struct Revision {
    revision_id: LooseString,
    uid: LooseString,
    created: LooseInt,
    #[serde(default)]
    late: LooseInt,
    #[serde(default)]
    draft: LooseInt,
    #[serde(default)]
    body: String,
    #[serde(default)]
    attachments: Attachments,
}

impl From<Revision> for Submission {
    fn from(revision: Revision) -> Self {
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
