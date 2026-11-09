use crate::api::schoology::account::SchoologyAccountConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{
    types::{LooseString, LooseUsize},
    schoology::RequestResult,
};
use foam::types::folder_info::FolderInfo;

const PAGE_LIMIT: usize = 50;

#[derive(Serialize, oauth::Request)]
struct FoldersQuery {
    start: usize,
    limit: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct FoldersResponse {
    section: Vec<SchoologyCourse>,
    total: LooseUsize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct SchoologyCourse {
    #[serde(rename = "id")]
    nid: LooseString,
    course_title: String,
    course_code: String,
    #[serde(alias = "title")]
    section_title: String,
    section_code: String,
    active: i64,
    description: String,
    #[serde(rename = "profile_url")]
    logo_img_src: String,
    location: String,
    meeting_days: Vec<Value>,
    start_time: String,
    end_time: String,
    weight: LooseString,
    links: Links,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Links {
    #[serde(rename = "self")]
    self_url: String,
}

impl SchoologyAccountConfig {
    pub fn section_folder_title(&self, id: &str) -> RequestResult<String> {
        let section: SchoologyCourse =
            self.api_get(&format!("https://api.schoology.com/v1/sections/{id}"))?;
        Ok(section.course_title)
    }

    /// Fetch every configured-user section and coerce it, including its complete
    /// resource tree, into unified folder models.
    pub fn scrape_folders(&self) -> RequestResult<Vec<FolderInfo>> {
        let folders = self.scrape_materials()?;
        // Grades were stored on the plugin's own table at course-scrape time
        // historically; restore that behaviour through the plugin storage.
        self.scrape_grades()?;
        Ok(folders)
    }

    /// Fetch sections and their resource trees.
    pub fn scrape_materials(&self) -> RequestResult<Vec<FolderInfo>> {
        let mut sections = Vec::new();
        let mut start = 0;
        loop {
            let query = FoldersQuery {
                start,
                limit: PAGE_LIMIT,
            };
            let mut page: FoldersResponse = self.api_get_with_query(
                &format!(
                    "https://api.schoology.com/v1/users/{}/sections",
                    self.user_id
                ),
                &query,
            )?;
            let page_len = page.section.len();
            let total = page.total.0;
            sections.append(&mut page.section);
            if page_len < PAGE_LIMIT || (total > 0 && sections.len() >= total) {
                break;
            }
            start += page_len;
        }

        sections
            .into_iter()
            .map(|section| {
                let hierarchy = self.course(&section.nid.0, "0")?;
                let folder = FolderInfo {
                    folder_id: section.nid.0,
                    aliases: Vec::new(),
                    title: section.course_title,
                    course_code: section.course_code,
                    course_url: section.links.self_url,
                    section_title: section.section_title,
                    section_code: section.section_code,
                    active: section.active != 0,
                    description: section.description,
                    logo_img_src: section.logo_img_src,
                    location: section.location,
                    period: None,
                    hidden: false,
                    folder_order: 0,
                    meeting_days: section
                        .meeting_days
                        .into_iter()
                        .filter_map(|day| match day {
                            Value::Number(n) => n.as_i64().and_then(|n| i8::try_from(n).ok()),
                            Value::String(s) => s.parse().ok(),
                            _ => None,
                        })
                        .collect(),
                    start_time: section.start_time,
                    end_time: section.end_time,
                    weight: section.weight.0,
                };
                foam::types::folder_info::store_folder(&folder, &hierarchy)?;
                Ok(folder)
            })
            .collect()
    }
}
