use serde::Deserialize;
use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use crate::{AppWindow, CoursesUi, SearchResult, SearchUi, database, filesystem};

#[derive(Deserialize)]
struct ResultRow {
    item_id: String,
    course_id: String,
    title: String,
    course_title: String,
    item_type: String,
    logo_img_src: String,
    preview: String,
}

fn match_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn search(input: &str) -> io::Result<Vec<ResultRow>> {
    let query = match_query(input);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    database::from_sql(include_str!("../sql/search.sql").into(), &[&query])
}

#[derive(Clone, Default)]
pub struct SearchState {
    generation: Arc<AtomicU64>,
}

impl SearchState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let global = ui.global::<SearchUi>();
        global.invoke_search(global.get_query());
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let (sender, receiver) = mpsc::channel::<(u64, String)>();
        let generation = self.generation.clone();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            while let Ok(mut request) = receiver.recv() {
                // 180 ms typing debounce
                while let Ok(next) = receiver.recv_timeout(Duration::from_millis(180)) {
                    request = next;
                }
                let (version, query) = request;
                if generation.load(Ordering::SeqCst) != version {
                    continue;
                }
                let result = search(&query);
                let generation = generation.clone();

                if weak
                    .upgrade_in_event_loop(move |ui| {
                        if generation.load(Ordering::SeqCst) != version {
                            return;
                        }
                        let global = ui.global::<SearchUi>();
                        global.set_busy(false);
                        match result {
                            Ok(rows) => {
                                global.set_error("".into());
                                global.set_results(ModelRc::new(VecModel::from(
                                    rows.into_iter()
                                        .map(|row| SearchResult {
                                            item_id: row.item_id.into(),
                                            course_id: row.course_id.into(),
                                            title: row.title.into(),
                                            course_title: row.course_title.into(),
                                            preview: row.preview.to_shared_string(),
                                            item_type: row.item_type.to_shared_string(),
                                            icon: Default::default(),
                                        })
                                        .collect::<Vec<_>>(),
                                )));
                                for row in rows.iter() {
                                    download_and_set_icon(&row.logo_img_src, row.item_id.clone());
                                }
                            }
                            Err(error) => {
                                log::warn!("Search failed: {error}");
                                global.set_results(ModelRc::default());
                                global.set_error("Could not search assignments. Try again.".into());
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let generation = self.generation.clone();
        let weak = ui.as_weak();
        ui.global::<SearchUi>().on_search(move |query| {
            let version = generation.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(ui) = weak.upgrade() {
                let global = ui.global::<SearchUi>();
                global.set_query(query.clone());
                global.set_results(ModelRc::default());
                global.set_error("".into());
                global.set_busy(!query.trim().is_empty());
            }
            let _ = sender.send((version, query.to_string()));
        });
    }
}

pub fn download_and_set_icon(url: &str, item_id: String) {
    if url.is_empty() {
        return;
    }

    filesystem::asset_as_slint_img(url, "", move |ui, result| match result {
        Ok(img) => {
            let search_results_model = ui.global::<SearchUi>().get_results();
            if let Some(index) = (0..search_results_model.row_count()).find(|&index| {
                search_results_model
                    .row_data(index)
                    .is_some_and(|data| data.item_id == item_id)
            }) && let Some(mut search_result) = search_results_model.row_data(index)
            {
                search_result.icon = img;
                search_results_model.set_row_data(index, search_result);
            }
        }
        Err(err) => log::warn!("Failed to load search result icon: {err}"),
    });
}
