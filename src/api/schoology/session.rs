//! Browser SSO recovery. WebKit objects stay on the UI thread; sync workers wait
//! for the result without blocking the event loop.
use crate::embedded_webview::EmbeddedWebView;
use std::{
    cell::RefCell,
    io,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use reqwest::{cookie::CookieStore, header::HeaderValue};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use wry::{
    Rect, WebContext, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

use super::{RequestResult, account::SchoologyAccountConfig, cookies::SessionCookies};
use crate::AppWindow;

static UI: OnceLock<slint::Weak<AppWindow>> = OnceLock::new();
static RECOVERY: Mutex<()> = Mutex::new(());
thread_local! {
    static CONTEXT: RefCell<WebContext> = RefCell::new(WebContext::new(Some(
        crate::config::config().data_dir().join("schoology-webview")
    )));
    static BROWSER: RefCell<Option<EmbeddedWebView>> = const { RefCell::new(None) };
}

pub(super) fn with_context<T>(f: impl FnOnce(&mut WebContext) -> T) -> T {
    CONTEXT.with(|context| f(&mut context.borrow_mut()))
}

pub(super) fn init(ui: &AppWindow) {
    let _ = UI.set(ui.as_weak());
}

pub(super) fn recover(
    account: &SchoologyAccountConfig,
    jar: &Arc<SessionCookies>,
    failed_cookie: Option<HeaderValue>,
) -> RequestResult<()> {
    let _guard = RECOVERY
        .lock()
        .map_err(|_| io::Error::other("Schoology recovery lock poisoned"))?;
    crate::thread_manager::check_cancelled()?;
    let url = reqwest::Url::parse(&format!("https://{}.schoology.com/", account.subdomain))?;
    // Another worker may already have recovered this shared client.
    if jar.cookies(&url) != failed_cookie {
        return Ok(());
    }
    let weak = UI
        .get()
        .ok_or_else(|| io::Error::other("Schoology recovery UI is not ready"))?;
    log::info!("Recovering Schoology session through browser sign-in");
    let host = url.host_str().unwrap().to_owned();
    let key = account.cookie_key.clone();
    let (tx, rx) = mpsc::channel();
    weak.upgrade_in_event_loop(move |ui| {
        let loaded = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let promoted = Arc::new(AtomicBool::new(false));
        let page_loaded = loaded.clone();
        let page_finished = finished.clone();
        let page_host = host.clone();
        let result = with_context(|context| {
            ui.window()
                .with_winit_window(|window| {
                    let size = ui.window().size();
                    let bounds = Rect {
                        position: LogicalPosition::new(24., 64.).into(),
                        size: LogicalSize::new(
                            (size.width as f64 / ui.window().scale_factor() as f64 - 48.).max(100.),
                            (size.height as f64 / ui.window().scale_factor() as f64 - 100.)
                                .max(100.),
                        )
                        .into(),
                    };
                    let builder = WebViewBuilder::new_with_web_context(context)
                        .with_on_page_load_handler(move |event, url| {
                            if matches!(event, wry::PageLoadEvent::Finished) {
                                page_finished.store(true, Ordering::Relaxed);
                                page_loaded
                                    .store(authenticated_home(&url, &page_host), Ordering::Relaxed);
                            }
                        })
                        .with_url(url.as_str());
                    EmbeddedWebView::build(builder, window, bounds, false)
                })
                .ok_or_else(|| io::Error::other("Schoology recovery window unavailable"))?
                .map_err(io::Error::other)
        });
        match result {
            Ok(webview) => {
                BROWSER.with(|browser| *browser.borrow_mut() = Some(webview));
                poll(host, key, tx, Instant::now(), loaded, finished, promoted);
            }
            Err(error) => {
                let _ = tx.send(Err(error));
            }
        }
    })?;
    loop {
        crate::thread_manager::check_cancelled()?;
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => {
                jar.replace_session(&result?)?;
                log::info!("Schoology session recovered");
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err(io::Error::other("Schoology recovery was closed").into()),
        }
    }
}

fn authenticated_home(url: &str, host: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https" && url.host_str() == Some(host) && url.path() == "/home"
    })
}

fn poll(
    host: String,
    key: String,
    tx: mpsc::Sender<io::Result<String>>,
    started: Instant,
    loaded: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    promoted: Arc<AtomicBool>,
) {
    slint::Timer::single_shot(Duration::from_millis(250), move || {
        let result = BROWSER.with(|browser| -> io::Result<Option<String>> {
            let browser = browser.borrow();
            let webview = browser
                .as_ref()
                .ok_or_else(|| io::Error::other("Schoology recovery closed"))?;
            if started.elapsed() > Duration::from_secs(120)
                || crate::thread_manager::check_cancelled().is_err()
            {
                return Err(io::Error::other(
                    "Schoology sign-in timed out. Try syncing again to sign in.",
                ));
            }
            let authenticated = if loaded.load(Ordering::Relaxed) {
                authenticated_home(&webview.url().map_err(io::Error::other)?, &host)
            } else {
                false
            };
            if authenticated {
                let cookies = webview
                    .cookies_for_url(&format!("https://{host}/"))
                    .map_err(io::Error::other)?;
                if let Some(cookie) = cookies
                    .iter()
                    .find(|cookie| cookie.name() == key && !cookie.value().is_empty())
                {
                    return Ok(Some(cookie.value().to_owned()));
                }
            }
            // There is no live store session, so the hidden webview is stuck on
            // a login page nobody can see. Promote it to a visible sign-in panel.
            if finished.load(Ordering::Relaxed) && !promoted.swap(true, Ordering::Relaxed) {
                log::info!("Schoology recovery needs interaction: showing sign-in panel");
                webview.set_visible(true).map_err(io::Error::other)?;
                let _ = webview.focus();
            }
            Ok(None)
        });
        match result {
            Ok(None) => poll(host, key, tx, started, loaded, finished, promoted),
            result => {
                BROWSER.with(|browser| {
                    browser.borrow_mut().take();
                });
                let _ = tx.send(result.and_then(|value| {
                    value.ok_or_else(|| io::Error::other("No Schoology session cookie"))
                }));
            }
        }
    });
}
