// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::ComponentHandle;
use std::error::Error;

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

#[cfg(target_os = "macos")]
use crate::platform::macos::macos_quit;
use crate::{config::config, state::state::state, ui::WEAK_UI};

mod account;
mod api;
mod config;
mod database;
mod filesystem;
mod platform;
mod plugin;
mod state;
mod thread_manager;
mod types;
mod ui;
mod wry;

slint::include_modules!();

fn shutdown() -> Result<(), Box<dyn std::error::Error>> {
    thread_manager::join_all_threads();
    config().save()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;

        slint::BackendSelector::new()
            .backend_name("winit".into())
            .with_winit_window_attributes_hook(|attributes| {
                attributes
                    .with_titlebar_transparent(true)
                    .with_fullsize_content_view(true)
                    .with_title_hidden(true)
            })
            .select()?;
    }
    #[cfg(not(target_os = "macos"))]
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;
    ui.set_custom_titlebar(cfg!(target_os = "macos"));

    *WEAK_UI.lock().expect("ui lock is poisoned") = Some(ui.as_weak());

    #[cfg(target_os = "macos")]
    let quit_target = macos_quit::new_target();

    let mut sidebar = state::sidebar::Sidebar::new(&ui);
    ui.window().on_winit_window_event(move |_window, event| {
        if sidebar.handle_event(event) == EventResult::PreventDefault {
            return EventResult::PreventDefault;
        }
        if let winit::event::WindowEvent::Focused(focus) = event
            && *focus
        {
            #[cfg(target_os = "macos")]
            macos_quit::on_focus(&quit_target);
            state().on_focus();
        }
        EventResult::Propagate
    });

    database::init()?;
    plugin::init()?;
    state().init();

    state().init_ui(&ui);
    state().sync_ui(&ui);
    api::schoology::onboarding::init(&ui);

    match ui.run() {
        Err(e) => log::warn!("Error in UI thread occured: {e}"),
        _ => log::info!("UI exited successfully"),
    }
    shutdown()?;

    log::info!("Shutdown successful");

    Ok(())
}
