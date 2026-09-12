// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

use crate::{
    config::config,
    state::state::{AppState, state},
};

mod api;
mod config;
mod filesystem;
mod state;
mod types;

slint::include_modules!();

fn shutdown() -> Result<(), Box<dyn std::error::Error>> {
    config().save()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;
    ui.window().on_winit_window_event(move |_window, event| {
        if let winit::event::WindowEvent::Focused(focus) = event
            && *focus
        {
            state().on_focus();
        }
        EventResult::Propagate
    });

    state().init();
    state().sync_ui(&ui);

    ui.run()?;
    shutdown()?;

    Ok(())
}
