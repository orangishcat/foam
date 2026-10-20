// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::ComponentHandle;
use std::error::Error;

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

use crate::{config::config, state::state::state, ui::WEAK_UI};

mod account;
mod api;
mod config;
mod database;
mod embedded_webview;
mod filesystem;
mod logging;
mod platform;
mod plugin;
mod state;
mod thread_manager;
mod types;
mod ui;
mod wry;

slint::include_modules!();

fn shutdown(ui: &AppWindow) -> Result<(), Box<dyn std::error::Error>> {
    thread_manager::join_all_threads();
    state().save(ui);
    config().save()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let _logger = logging::init(&config::default_data_dir())?;

    crate::platform::init()?;
    let ui = AppWindow::new()?;
    crate::platform::setup_window(&ui);

    *WEAK_UI.lock().expect("ui lock is poisoned") = Some(ui.as_weak());

    let mut sidebar = state::sidebar::Sidebar::new(&ui);
    ui.window().on_winit_window_event(move |_window, event| {
        crate::platform::on_window_event(_window, event);
        if sidebar.handle_event(event) == EventResult::PreventDefault {
            return EventResult::PreventDefault;
        }
        if let winit::event::WindowEvent::Focused(focus) = event
            && *focus
        {
            crate::platform::on_focus_gained();
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

    let _event_pump = crate::platform::start_event_pump();

    match ui.run() {
        Err(e) => log::warn!("Error in UI thread occured: {e}"),
        _ => log::info!("UI exited successfully"),
    }
    shutdown(&ui)?;

    log::info!("Shutdown successful");

    Ok(())
}
