use crate::{
    AppWindow, AssignmentUi, CoursesUi, MaterialItem,
    Screen::{self},
    UiState,
    state::courses::{children, course_folders, course_title, folder_metadata, root_folder},
};
use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel};
use std::io;

pub fn get_browser_data(
    ui: &AppWindow,
) -> Result<
    (
        std::string::String,
        std::string::String,
        Vec<MaterialItem>,
        Vec<MaterialItem>,
    ),
    std::io::Error,
> {
    let courses_ui = ui.global::<CoursesUi>();
    let course = courses_ui.get_course_id().to_string();
    let folder = courses_ui.get_folder_id().to_string();
    if course.is_empty() {
        return Ok((
            "Courses".to_owned(),
            "Courses".to_owned(),
            course_folders(ui),
            course_folders(ui),
        ));
    }
    let course_title = crate::types::course::course(&course)?
        .ok_or_else(|| io::Error::other("Course no longer exists"))?
        .course_title;
    let root = root_folder(&course)?;
    if folder.is_empty() {
        return Ok((
            course_title,
            "Courses".to_owned(),
            children(&course, &root)?,
            course_folders(ui),
        ));
    }
    let (title, parent) = folder_metadata(&course, &folder)?;
    let parent_title = if parent == root {
        course_title
    } else {
        folder_metadata(&course, &parent)?.0
    };
    Ok((
        title,
        parent_title,
        children(&course, &folder)?,
        children(&course, &parent)?,
    ))
}

pub fn refresh_file_view(ui: &AppWindow) {
    let courses_ui = ui.global::<CoursesUi>();
    let (doc_id, doc_name, doc_kind) = {
        let screen = ui.global::<UiState>().get_screen();
        if screen == Screen::Assignment {
            let assign_model = ui.global::<AssignmentUi>().get_assignment();
            (
                assign_model.id,
                assign_model.title,
                "assignment".to_shared_string(),
            )
        } else if screen == Screen::Document {
            // todo
            (
                "".to_shared_string(),
                "".to_shared_string(),
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

    courses_ui.set_browser_error("".into());
    match get_browser_data(ui) {
        Ok((title, parent, items, siblings)) => {
            courses_ui.set_folder_title(title.into());
            courses_ui.set_parent_title(parent.into());
            courses_ui.set_materials(ModelRc::new(VecModel::from(items)));
            courses_ui.set_parent_materials(ModelRc::new(VecModel::from(siblings)));
        }
        Err(err) => {
            log::warn!("Loading materials failed: {err}");
            courses_ui.set_browser_error("Unable to load materials.".into());
            courses_ui.set_materials(ModelRc::default());
            courses_ui.set_parent_materials(ModelRc::default());
        }
    }
    if let Ok(index) = usize::try_from(courses_ui.get_active_tab()) {
        let tabs = courses_ui.get_tabs();
        if let Some(mut tab) = tabs.row_data(index) {
            tab.course_id = courses_ui.get_course_id();
            tab.title = course_title(ui, &tab.course_id);
            tab.folder_id = courses_ui.get_folder_id();
            tab.folder_title = courses_ui.get_folder_title();
            tab.parent_title = courses_ui.get_parent_title();
            tab.browser_error = courses_ui.get_browser_error();
            tab.materials = courses_ui.get_materials();
            tab.parent_materials = courses_ui.get_parent_materials();
            match breadcrumbs(
                &tab.course_id,
                &tab.folder_id,
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
    course: &str,
    folder: &str,
    doc_id: &str,
    doc_name: &str,
    doc_kind: &str,
    title: slint::SharedString,
) -> io::Result<Vec<MaterialItem>> {
    let root = root_folder(course)?;
    let mut ancestors = Vec::new();
    let mut current = folder.to_owned();
    let mut visited = std::collections::HashSet::new();
    if !doc_name.is_empty() {
        ancestors.push(MaterialItem {
            course_id: course.into(),
            id: doc_id.into(),
            kind: doc_kind.into(),
            title: doc_name.into(),
        });
    }
    while !current.is_empty() && current != root {
        if !visited.insert(current.clone()) {
            return Err(io::Error::other("Folder hierarchy contains a cycle"));
        }
        let (folder_title, parent) = folder_metadata(course, &current)?;
        ancestors.push(MaterialItem {
            course_id: course.into(),
            id: current.into(),
            title: folder_title.into(),
            kind: "folder".into(),
        });
        current = parent;
    }
    ancestors.push(MaterialItem {
        course_id: course.into(),
        id: "".into(),
        title,
        kind: "course".into(),
    });
    ancestors.reverse();
    Ok(ancestors)
}
