use crate::api::schoology::account::SchoologyAccountConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{
    types::{LooseString, LooseUsize},
    schoology::RequestResult,
};
use foam::types::folder_info::FolderData;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    pub fn scrape_folders(&self) -> RequestResult<Vec<FolderData>> {
        crate::api::schoology::plugin_data::ensure_table()?;
        crate::api::schoology::plugin_data::ensure_scrape_version()?;
        let folders = self.scrape_materials()?;
        // Grades were stored on the plugin's own table at course-scrape time
        // historically; restore that behaviour through the plugin storage.
        self.scrape_grades()?;
        Ok(folders)
    }

    /// Fetch sections and their resource trees.
    pub fn scrape_materials(&self) -> RequestResult<Vec<FolderData>> {
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

        let mut completed = crate::api::schoology::plugin_data::completed_scrape_folders()?;
        let mut scraped = Vec::new();
        for section in sections {
            let folder_id = section.nid.0.clone();
            if completed.contains(&folder_id) {
                continue;
            }
            // Full Schoology-owned section metadata for the plugin's own
            // materials table; core keeps only the generic folder data.
            let material = serde_json::to_value(&section)?;
            let hierarchy = self.course(&folder_id, "0")?;
            let folder = FolderData {
                folder_id: folder_id.clone(),
                title: section.course_title,
                custom_icon: section.logo_img_src,
                description: section.description,
                ..Default::default()
            };
            crate::api::schoology::plugin_data::store_material("course", &folder_id, &material)?;
            foam::types::folder_info::store_folder(&folder, &hierarchy)?;
            crate::api::schoology::plugin_data::mark_scrape_folder_completed(&folder_id)?;
            completed.insert(folder_id);
            scraped.push(folder);
        }
        crate::api::schoology::plugin_data::finish_scrape()?;
        Ok(scraped)
    }
}
