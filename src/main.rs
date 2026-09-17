// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

#[cfg(target_os = "macos")]
use crate::platform::macos::macos_quit;
use crate::{config::config, state::state::state, ui::WEAK_UI};

mod api;
mod config;
mod database;
mod filesystem;
mod platform;
mod state;
mod thread_manager;
mod types;
mod ui;

slint::include_modules!();

fn shutdown() -> Result<(), Box<dyn std::error::Error>> {
    thread_manager::join_all_threads();
    config().save()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;

    *WEAK_UI.lock().expect("ui lock is poisoned") = Some(ui.as_weak());

    #[cfg(target_os = "macos")]
    let quit_target = macos_quit::new_target();

    ui.window().on_winit_window_event(move |_window, event| {
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
    state().init();
    let snapshot = state().clone();
    snapshot.sync_ui(&ui);

    match ui.run() {
        Err(e) => log::warn!("Error in UI thread occured: {e}"),
        _ => log::info!("UI exited successfully"),
    }
    shutdown()?;

    log::info!("Shutdown successful");

    Ok(())
}
