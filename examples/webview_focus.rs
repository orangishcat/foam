//! Linux focus reproducer, independent of Foam's onboarding and account state.
//! See docs/webview-focus-investigation.md for the experiment matrix.
#[cfg(target_os = "linux")]
#[path = "../src/platform/linux.rs"]
#[allow(dead_code)]
pub(crate) mod linux_backend;
#[cfg(target_os = "linux")]
use linux_backend::{webview_diagnostics as diagnostics, webview_focus, x11_interop};
#[cfg(target_os = "linux")]
mod platform {
    pub(crate) use crate::linux_backend as linux;
}
#[cfg(target_os = "linux")]
#[path = "../src/embedded_webview.rs"]
mod embedded_webview;

#[cfg(target_os = "linux")]
fn builder(url: Option<&str>) -> wry::WebViewBuilder<'static> {
    let builder = diagnostics::instrument(wry::WebViewBuilder::new());
    match url {
        Some(url) => builder.with_url(url.to_owned()),
        None => builder.with_html(diagnostics::TEST_HTML),
    }
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use gtk::prelude::*;
    use slint::winit_030::winit::{
        self,
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        platform::x11::EventLoopBuilderExtX11,
        window::{Window, WindowId},
    };
    use wry::WebViewBuilderExtUnix;

    let _logger = flexi_logger::Logger::try_with_env_or_str("info")?
        .log_to_stderr()
        .start()?;
    gtk::gdk::set_allowed_backends("x11");
    gtk::init()?;
    x11_interop::install_x11_error_hook(
        std::env::var("FOAM_X11_ERROR_WORKAROUND").as_deref() != Ok("0"),
    )?;
    let url = std::env::args().nth(2);
    match std::env::args().nth(1).as_deref().unwrap_or("winit") {
        "slint" => return run_slint_probe(url.as_deref()),
        "gtk" => {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title("Foam GTK focus probe");
            window.set_default_size(800, 600);
            let webview = builder(url.as_deref()).build_gtk(&window)?;
            diagnostics::attach(&webview);
            window.connect_delete_event(|_, _| {
                gtk::main_quit();
                gtk::glib::Propagation::Proceed
            });
            window.show_all();
            gtk::main();
            drop(webview);
        }
        "winit" => {
            #[derive(Default)]
            struct Probe {
                webview: Option<embedded_webview::EmbeddedWebView>,
                background: Option<embedded_webview::EmbeddedWebView>,
                window: Option<Window>,
                url: Option<String>,
                hidden: bool,
                commands: Option<std::sync::mpsc::Receiver<String>>,
            }
            impl Probe {
                fn command(&mut self, command: &str) {
                    let Some(window) = &self.window else {
                        return;
                    };
                    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                    let bounds = wry::Rect {
                        position: wry::dpi::LogicalPosition::new(0., 0.).into(),
                        size: wry::dpi::LogicalSize::new(size.width, size.height).into(),
                    };
                    match command {
                        "hide" | "show" => {
                            self.hidden = command == "hide";
                            self.webview
                                .as_ref()
                                .unwrap()
                                .set_visible(!self.hidden)
                                .unwrap();
                        }
                        "replace" => {
                            self.webview = None;
                            let webview = embedded_webview::EmbeddedWebView::build(
                                builder(self.url.as_deref()),
                                window,
                                bounds,
                                !self.hidden,
                            )
                            .unwrap();
                            diagnostics::attach(&webview);
                            self.webview = Some(webview);
                        }
                        "background" => {
                            if self.background.is_some() {
                                self.background = None;
                            } else {
                                self.background = Some(
                                    embedded_webview::EmbeddedWebView::build(
                                        builder(None),
                                        window,
                                        bounds,
                                        false,
                                    )
                                    .unwrap(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            impl ApplicationHandler for Probe {
                fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                    if self.window.is_some() {
                        return;
                    }
                    let window = event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title("Foam winit focus probe")
                                .with_inner_size(winit::dpi::LogicalSize::new(800., 600.)),
                        )
                        .unwrap();
                    let webview = embedded_webview::EmbeddedWebView::build(
                        builder(self.url.as_deref()),
                        &window,
                        wry::Rect {
                            position: wry::dpi::LogicalPosition::new(0., 0.).into(),
                            size: wry::dpi::LogicalSize::new(800., 600.).into(),
                        },
                        true,
                    )
                    .unwrap();
                    diagnostics::attach(&webview);
                    webview.set_visible(true).unwrap();
                    self.webview = Some(webview);
                    self.window = Some(window);
                }
                fn window_event(
                    &mut self,
                    event_loop: &ActiveEventLoop,
                    _: WindowId,
                    event: WindowEvent,
                ) {
                    if let Some(window) = &self.window {
                        webview_focus::window_event(window, &event);
                    }
                    match event {
                        WindowEvent::Focused(focused) => {
                            if let Some(window) = &self.window {
                                diagnostics::trace_parent_focus(window, focused);
                            }
                        }
                        WindowEvent::Resized(size) => {
                            if let (Some(webview), Some(window)) = (&self.webview, &self.window) {
                                let size = size.to_logical::<f64>(window.scale_factor());
                                webview
                                    .set_bounds(wry::Rect {
                                        position: wry::dpi::LogicalPosition::new(0., 0.).into(),
                                        size: wry::dpi::LogicalSize::new(size.width, size.height)
                                            .into(),
                                    })
                                    .unwrap();
                            }
                        }
                        WindowEvent::CloseRequested => event_loop.exit(),
                        _ => {}
                    }
                }
                fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
                    webview_focus::sync_native_focus();
                    let commands: Vec<_> = self
                        .commands
                        .as_ref()
                        .map(|rx| rx.try_iter().collect())
                        .unwrap_or_default();
                    for command in commands {
                        self.command(&command);
                    }
                    while gtk::events_pending() {
                        gtk::main_iteration_do(false);
                    }
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + std::time::Duration::from_millis(10),
                    ));
                }
            }
            let event_loop = EventLoop::builder().with_x11().build()?;
            log::info!(
                "Probe stdin commands: hide, show, replace, background (toggle hidden recovery view)"
            );
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                    if tx.send(line.trim().to_owned()).is_err() {
                        break;
                    }
                }
            });
            // Continue servicing GTK even when winit has no pending input.
            // Match Foam's 10 ms pumping interval.
            event_loop.run_app(&mut Probe {
                url,
                commands: Some(rx),
                ..Probe::default()
            })?;
        }
        _ => return Err("Expected host: winit, gtk, or slint".into()),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_slint_probe(url: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
    slint::slint! {
        import { LineEdit } from "std-widgets.slint";
        export component FocusProbe inherits Window {
            width: 1100px; height: 800px;
            title: "Foam Slint focus probe";
            in-out property <string> entry;
            callback entry-changed();
            changed entry => { root.entry-changed(); }
            Text { x: 20px; y: 20px; text: "Slint input"; }
            LineEdit { x: 20px; y: 60px; width: 300px; height: 40px; text <=> root.entry; }
            Text { x: 20px; y: 130px; text: "Slint value: " + root.entry; }
        }
    }
    let mut event_loop = slint::winit_030::winit::event_loop::EventLoop::with_user_event();
    use slint::winit_030::winit::platform::x11::EventLoopBuilderExtX11;
    event_loop.with_x11();
    linux_backend::select(event_loop)?;
    let ui = FocusProbe::new()?;
    ui.on_entry_changed(|| log::info!(target: "foam::webview", "Slint input changed"));
    let view: std::rc::Rc<std::cell::RefCell<Option<embedded_webview::EmbeddedWebView>>> =
        Default::default();
    let slot = view.clone();
    let weak = ui.as_weak();
    let url = url.map(str::to_owned);
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        if let Some(ui) = weak.upgrade() {
            let webview = ui
                .window()
                .with_winit_window(|window| {
                    embedded_webview::EmbeddedWebView::build(
                        builder(url.as_deref()),
                        window,
                        wry::Rect {
                            position: wry::dpi::LogicalPosition::new(400., 0.).into(),
                            size: wry::dpi::LogicalSize::new(700., 800.).into(),
                        },
                        true,
                    )
                })
                .expect("Probe native window unavailable")
                .unwrap();
            diagnostics::attach(&webview);
            *slot.borrow_mut() = Some(webview);
        }
    });
    ui.window().on_winit_window_event(move |window, event| {
        if let slint::winit_030::winit::event::WindowEvent::Resized(_) = event
            && let Some(view) = view.borrow().as_ref()
        {
            let size = window.size();
            let scale = window.scale_factor() as f64;
            view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(400., 0.).into(),
                size: wry::dpi::LogicalSize::new(
                    (size.width as f64 / scale - 400.).max(1.),
                    size.height as f64 / scale,
                )
                .into(),
            })
            .unwrap();
        }
        slint::winit_030::EventResult::Propagate
    });
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
    ui.run()?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This focus reproducer is Linux-only.");
}
