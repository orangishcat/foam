use crate::api::{
    schoology::{RequestResult, account::SchoologyAccountConfig},
    types::{LooseString, LooseUsize},
};
use serde::{Deserialize, Serialize};

const PAGE_LIMIT: usize = 50;

#[derive(Serialize, oauth::Request)]
struct EventsQuery {
    start: usize,
    limit: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct EventsResponse {
    event: Vec<SectionEvent>,
    total: LooseUsize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct SectionEvent {
    id: LooseString,
}

impl SchoologyAccountConfig {
    /// Lists the IDs of every event in a section. The section is the realm that event API calls require.
    pub fn scrape_section_event_ids(&self, course_id: &str) -> RequestResult<Vec<String>> {
        let url = format!("https://api.schoology.com/v1/sections/{course_id}/events");
        let mut ids = Vec::new();
        let mut start = 0;
        loop {
            let query = EventsQuery {
                start,
                limit: PAGE_LIMIT,
            };
            let page: EventsResponse = self.api_get_with_query(&url, &query)?;
            let page_len = page.event.len();
            let total = page.total.0;
            ids.extend(page.event.into_iter().map(|event| event.id.0));
            if page_len < PAGE_LIMIT || (total > 0 && ids.len() >= total) {
                break;
            }
            start += page_len;
        }
        Ok(ids)
    }
}
