use crate::{
    AppWindow, TaskUi, FoldersUi, ResourceItem,
    Screen::{self},
    UiState,
    state::folders::FolderState,
};
use anyhow;
use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel};
use std::io;

pub fn get_path_data(
    ui: &AppWindow,
) -> anyhow::Result<(
    std::string::String,
    std::string::String,
    Vec<ResourceItem>,
    Vec<ResourceItem>,
)> {
    let folders_ui = ui.global::<FoldersUi>();
    let folder = folders_ui.get_folder_id().to_string();
    let resource_folder = folders_ui.get_resource_folder_id().to_string();
    if folder.is_empty() {
        return Ok((
            "Folders".to_owned(),
            "Folders".to_owned(),
            FolderState::folder_list_items(ui),
            FolderState::folder_list_items(ui),
        ));
    }
    let folder_title = crate::types::folder_info::folder(&folder)?
        .ok_or_else(|| io::Error::other("Folder no longer exists"))?
        .title;
    let root = FolderState::root_folder(&folder)?;
    if resource_folder.is_empty() {
        return Ok((
            folder_title,
            "Folders".to_owned(),
            FolderState::children(ui, &folder, &root)?,
            FolderState::folder_list_items(ui),
        ));
    }
    let (title, parent) = FolderState::folder_metadata(&folder, &resource_folder)?;
    let parent_title = if parent == root {
        folder_title
    } else {
        FolderState::folder_metadata(&folder, &parent)?.0
    };
    Ok((
        title,
        parent_title,
        FolderState::children(ui, &folder, &resource_folder)?,
        FolderState::children(ui, &folder, &parent)?,
    ))
}

pub fn refresh_file_view(ui: &AppWindow) {
    restore_file_view(ui);
}

pub(crate) fn restore_file_view(ui: &AppWindow) {
    let folders_ui = ui.global::<FoldersUi>();
    let (doc_id, doc_name, doc_kind) = {
        let screen = ui.global::<UiState>().get_screen();
        if screen == Screen::Task {
            let task_model = ui.global::<TaskUi>().get_task();
            (task_model.id, task_model.title, "task".to_shared_string())
        } else if screen == Screen::Document {
            (
                ui.global::<crate::DocumentUi>().get_id(),
                ui.global::<crate::DocumentUi>().get_title(),
                "document".to_shared_string(),
            )
        } else {
            (
                "".to_shared_string(),
                "".to_shared_string(),
                "".to_shared_string(),
            )
        }
    };

    folders_ui.set_browser_error("".into());
    match get_path_data(ui) {
        Ok((title, parent, items, siblings)) => {
            folders_ui.set_resource_folder_title(title.into());
            folders_ui.set_parent_title(parent.into());
            folders_ui.set_resources(ModelRc::new(VecModel::from(items)));
            folders_ui.set_parent_resources(ModelRc::new(VecModel::from(siblings)));
        }
        Err(err) => {
            log::warn!("Loading resources failed: {err}");
            folders_ui.set_browser_error("Unable to load resources.".into());
            folders_ui.set_resources(ModelRc::default());
            folders_ui.set_parent_resources(ModelRc::default());
        }
    }
    if let Ok(index) = usize::try_from(folders_ui.get_active_tab()) {
        let tabs = folders_ui.get_tabs();
        if let Some(mut tab) = tabs.row_data(index) {
            tab.folder_id = folders_ui.get_folder_id();
            tab.title = FolderState::folder_title(ui, &tab.folder_id);
            tab.resource_folder_id = folders_ui.get_resource_folder_id();
            tab.resource_folder_title = folders_ui.get_resource_folder_title();
            tab.parent_title = folders_ui.get_parent_title();
            tab.browser_error = folders_ui.get_browser_error();
            tab.resources = folders_ui.get_resources();
            tab.parent_resources = folders_ui.get_parent_resources();
            match breadcrumbs(
                &tab.folder_id,
                &tab.resource_folder_id,
                &doc_id,
                &doc_name,
                &doc_kind,
                tab.title.clone(),
            ) {
                Ok(path) => tab.breadcrumbs = ModelRc::new(VecModel::from(path)),
                Err(err) => {
                    log::warn!("Loading folder path failed: {err}");
                    tab.breadcrumbs = ModelRc::default();
                }
            }
            tabs.set_row_data(index, tab);
        }
    }
}

pub fn breadcrumbs(
    folder: &str,
    resource_folder: &str,
    doc_id: &str,
    doc_name: &str,
    doc_kind: &str,
    title: slint::SharedString,
) -> anyhow::Result<Vec<ResourceItem>> {
    let root = FolderState::root_folder(folder)?;
    let mut ancestors = Vec::new();
    let mut current = resource_folder.to_owned();
    let mut visited = std::collections::HashSet::new();
    if !doc_name.is_empty() {
        ancestors.push(ResourceItem {
            folder_id: folder.into(),
            id: doc_id.into(),
            kind: doc_kind.into(),
            title: doc_name.into(),
            icon_color: Default::default(),
        });
    }
    while !current.is_empty() && current != root {
        if !visited.insert(current.clone()) {
            return Err(io::Error::other("Folder hierarchy contains a cycle").into());
        }
        let (folder_title, parent) = FolderState::folder_metadata(folder, &current)?;
        ancestors.push(ResourceItem {
            folder_id: folder.into(),
            id: current.into(),
            title: folder_title.into(),
            kind: "folder".into(),
            icon_color: Default::default(),
        });
        current = parent;
    }
    ancestors.push(ResourceItem {
        folder_id: folder.into(),
        id: "".into(),
        title,
        kind: "folder".into(),
        icon_color: Default::default(),
    });
    ancestors.reverse();
    Ok(ancestors)
}
