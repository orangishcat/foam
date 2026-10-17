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
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;

        // Wry child webviews require X11, including GTK's display connection.
        // Wayland desktops run this window through XWayland.
        gtk::gdk::set_allowed_backends("x11");
        gtk::init()?;
        crate::platform::linux::x11_interop::install_x11_error_hook(true)?;
        let mut event_loop_builder = winit::event_loop::EventLoop::with_user_event();
        event_loop_builder.with_x11();
        crate::platform::linux::select(event_loop_builder)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;
    ui.set_custom_titlebar(cfg!(any(target_os = "macos", target_os = "linux")));
    ui.global::<WindowChrome>()
        .set_client_side(cfg!(target_os = "linux"));

    #[cfg(target_os = "linux")]
    {
        let weak = ui.as_weak();
        crate::platform::linux::webview_focus::on_activation_change(move |id, active| {
            if let Some(ui) = weak.upgrade() {
                ui.window().with_winit_window(|window| {
                    if window.id() == id {
                        ui.set_window_active(active);
                    }
                });
            }
        });
    }

    #[cfg(target_os = "linux")]
    {
        let weak_ui = ui.as_weak();
        ui.global::<WindowChrome>().on_toggle_maximized(move || {
            if let Some(ui) = weak_ui.upgrade() {
                let window = ui.window();
                if !window.is_fullscreen() {
                    window.set_maximized(!window.is_maximized());
                }
            }
        });
    }

    *WEAK_UI.lock().expect("ui lock is poisoned") = Some(ui.as_weak());

    #[cfg(target_os = "macos")]
    let quit_target = macos_quit::new_target();

    let mut sidebar = state::sidebar::Sidebar::new(&ui);
    #[cfg(target_os = "linux")]
    let titlebar_ui = ui.as_weak();
    ui.window().on_winit_window_event(move |_window, event| {
        #[cfg(target_os = "linux")]
        if crate::platform::linux::webview_diagnostics::enabled()
            && let winit::event::WindowEvent::Focused(focused) = event
        {
            _window.with_winit_window(|window| {
                crate::platform::linux::webview_diagnostics::trace_parent_focus(window, *focused);
            });
        }
        #[cfg(target_os = "linux")]
        if let winit::event::WindowEvent::Focused(focus) = event
            && let Some(ui) = titlebar_ui.upgrade()
        {
            let active = _window
                .with_winit_window(|window| {
                    crate::platform::linux::webview_focus::parent_active(window.id())
                        .unwrap_or(*focus)
                })
                .unwrap_or(*focus);
            ui.set_window_active(active);
        }
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

    #[cfg(target_os = "linux")]
    let _gtk_timer = {
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(10),
            || {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            },
        );
        timer
    };

    match ui.run() {
        Err(e) => log::warn!("Error in UI thread occured: {e}"),
        _ => log::info!("UI exited successfully"),
    }
    shutdown(&ui)?;

    log::info!("Shutdown successful");

    Ok(())
}
