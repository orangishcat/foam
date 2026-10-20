//! Focus ownership for GTK webviews embedded in winit's X11 windows.
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
};

use gtk::{
    glib::{self, translate::*},
    prelude::*,
};
use slint::winit_030::{
    EventResult,
    winit::{
        self,
        event::{ElementState, WindowEvent},
        window::{Window, WindowId},
    },
};
use wry::{Rect, WebView, WebViewExtUnix};

struct View {
    parent: WindowId,
    widget: glib::WeakRef<gtk::Widget>,
    toplevel: glib::WeakRef<gtk::Window>,
    bounds: Rect,
    visible: bool,
    focused: bool,
    order: u64,
    native_child: x11_dl::xlib::Window,
    key_masks: (gtk::gdk::EventMask, gtk::gdk::EventMask),
}

#[derive(Clone, Copy)]
struct NativeParent {
    display: usize,
    window: x11_dl::xlib::Window,
}

impl NativeParent {
    fn focused_window(self) -> Option<x11_dl::xlib::Window> {
        XLIB.try_with(|xlib| {
            let xlib = xlib.as_ref()?;
            let (mut window, mut revert) = (0, 0);
            // SAFETY: GTK owns this display for the lifetime of the UI thread.
            unsafe {
                (xlib.XGetInputFocus)(self.display as *mut _, &mut window, &mut revert);
            }
            Some(window)
        })
        .ok()
        .flatten()
    }

    fn focus(self, window: x11_dl::xlib::Window) {
        let _ = XLIB.try_with(|xlib| {
            if let Some(xlib) = xlib {
                // Only live, registered child/parent windows are passed here.
                unsafe {
                    (xlib.XSetInputFocus)(
                        self.display as *mut _,
                        window,
                        x11_dl::xlib::RevertToParent,
                        x11_dl::xlib::CurrentTime,
                    );
                    (xlib.XFlush)(self.display as *mut _);
                }
            }
        });
    }
}

#[derive(Default)]
struct Parent {
    active: bool,
    selected: Option<u64>,
    cursor: Option<winit::dpi::PhysicalPosition<f64>>,
    scale: f64,
    native: Option<NativeParent>,
}

#[derive(Default)]
struct Manager {
    sequence: u64,
    views: BTreeMap<u64, View>,
    parents: HashMap<WindowId, Parent>,
}

struct FocusChange {
    widget: glib::WeakRef<gtk::Widget>,
    toplevel: glib::WeakRef<gtk::Window>,
    focused: bool,
    native: Option<NativeParent>,
    child: x11_dl::xlib::Window,
    parent_active: bool,
    key_masks: (gtk::gdk::EventMask, gtk::gdk::EventMask),
}

fn keyboard_events(
    widget: &gtk::Widget,
    toplevel: &gtk::Window,
    masks: (gtk::gdk::EventMask, gtk::gdk::EventMask),
    focused: bool,
) {
    let keys = gtk::gdk::EventMask::KEY_PRESS_MASK | gtk::gdk::EventMask::KEY_RELEASE_MASK;
    for (window, original) in [(widget.window(), masks.0), (toplevel.window(), masks.1)] {
        if let Some(window) = window {
            let events = window.events() & !keys;
            window.set_events(if focused {
                events | (original & keys)
            } else {
                events
            });
        }
    }
}

thread_local! { static MANAGER: RefCell<Manager> = RefCell::default(); }
thread_local! {
    static XLIB: Option<x11_dl::xlib::Xlib> = x11_dl::xlib::Xlib::open().ok();
    static ACTIVATION: RefCell<Option<Box<dyn Fn(WindowId, bool)>>> = RefCell::new(None);
}

pub(crate) fn on_activation_change(callback: impl Fn(WindowId, bool) + 'static) {
    ACTIVATION.with(|slot| *slot.borrow_mut() = Some(Box::new(callback)));
}

pub(crate) fn parent_active(id: WindowId) -> Option<bool> {
    MANAGER
        .try_with(|slot| slot.borrow().parents.get(&id).map(|p| p.active))
        .ok()
        .flatten()
}

fn native_active(id: WindowId) -> Option<bool> {
    let native = MANAGER
        .try_with(|slot| slot.borrow().parents.get(&id).and_then(|p| p.native))
        .ok()
        .flatten()?;
    let focused = native.focused_window()?;
    MANAGER
        .try_with(|slot| {
            let manager = slot.borrow();
            manager.contains_native_focus(id, focused)
        })
        .ok()
}

/// winit stops reporting parent activation once X11 focus is in a native child.
/// Sample registered parents using the existing GTK pump cadence.
pub(crate) fn sync_native_focus() {
    let parents = MANAGER
        .try_with(|slot| {
            slot.borrow()
                .parents
                .iter()
                .filter(|(_, p)| p.native.is_some())
                .map(|(&id, _)| id)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for id in parents {
        if let Some(active) = native_active(id)
            && parent_active(id) != Some(active)
        {
            change(|manager| {
                if let Some(parent) = manager.parents.get_mut(&id) {
                    parent.active = active;
                }
            });
            let _ = ACTIVATION.try_with(|slot| {
                if let Some(callback) = slot.borrow().as_ref() {
                    callback(id, active);
                }
            });
        }
    }
}

impl Manager {
    fn contains_native_focus(&self, parent: WindowId, focused: x11_dl::xlib::Window) -> bool {
        self.parents
            .get(&parent)
            .and_then(|p| p.native)
            .is_some_and(|native| {
                focused == native.window
                    || self
                        .views
                        .values()
                        .any(|v| v.parent == parent && v.visible && v.native_child == focused)
            })
    }

    fn owns_keyboard(&self, id: u64) -> bool {
        self.views.get(&id).is_some_and(|view| {
            view.visible
                && self
                    .parents
                    .get(&view.parent)
                    .is_some_and(|p| p.active && p.selected == Some(id))
        })
    }

    fn select(&mut self, parent: WindowId, id: Option<u64>) {
        self.parents.entry(parent).or_default().selected = id;
    }

    fn hide(&mut self, id: u64) {
        if let Some(view) = self.views.get_mut(&id) {
            view.visible = false;
            if let Some(parent) = self.parents.get_mut(&view.parent) {
                if parent.selected == Some(id) {
                    parent.selected = None;
                }
            }
        }
    }

    fn hit(&self, parent: WindowId) -> Option<u64> {
        let p = self.parents.get(&parent)?;
        let cursor = p.cursor?;
        self.views
            .iter()
            .filter(|(_, view)| {
                if view.parent != parent || !view.visible {
                    return false;
                }
                let position = view.bounds.position.to_physical::<f64>(p.scale);
                let size = view.bounds.size.to_physical::<f64>(p.scale);
                cursor.x >= position.x
                    && cursor.y >= position.y
                    && cursor.x < position.x + size.width
                    && cursor.y < position.y + size.height
            })
            .max_by_key(|(_, view)| view.order)
            .map(|(&id, _)| id)
    }

    fn changes(&mut self) -> Vec<FocusChange> {
        let desired: Vec<_> = self
            .views
            .keys()
            .map(|&id| (id, self.owns_keyboard(id)))
            .collect();
        let mut changes = Vec::new();
        for (id, focused) in desired {
            let view = self.views.get_mut(&id).unwrap();
            if focused != view.focused {
                view.focused = focused;
                changes.push(FocusChange {
                    widget: view.widget.clone(),
                    toplevel: view.toplevel.clone(),
                    focused,
                    native: self.parents.get(&view.parent).and_then(|p| p.native),
                    child: view.native_child,
                    parent_active: self.parents.get(&view.parent).is_some_and(|p| p.active),
                    key_masks: view.key_masks,
                });
            }
        }
        changes.sort_by_key(|change| change.focused); // Blur the old view first.
        changes
    }
}

fn change(update: impl FnOnce(&mut Manager)) {
    let changes = MANAGER
        .try_with(|slot| {
            let mut manager = slot.borrow_mut();
            update(&mut manager);
            manager.changes()
        })
        .unwrap_or_default();
    // GTK can call back synchronously. Never hold the manager borrow here.
    for change in changes {
        change.apply();
    }
}

impl FocusChange {
    fn apply(self) {
        let (Some(widget), Some(toplevel)) = (self.widget.upgrade(), self.toplevel.upgrade())
        else {
            return;
        };
        // An unfocused GTK child must not intercept keys merely because the
        // pointer is hovering over it. Let X11 propagate them to Slint's parent.
        keyboard_events(&widget, &toplevel, self.key_masks, self.focused);
        if self.focused {
            toplevel.set_focus(Some(&widget));
        }
        let mut event = gtk::gdk::Event::new(gtk::gdk::EventType::FocusChange);
        if let Some(keyboard) = gtk::gdk::Display::default()
            .and_then(|d| d.default_seat())
            .and_then(|s| s.keyboard())
        {
            event.set_device(Some(&keyboard));
        }
        // SAFETY: this event was allocated as GDK_FOCUS_CHANGE. Transfer a new
        // window reference to the event; gdk_event_free releases it on drop.
        unsafe {
            let raw: *mut gtk::gdk::ffi::GdkEvent = event.to_glib_none_mut().0;
            (*raw).focus_change.in_ = i16::from(self.focused);
            (*raw).focus_change.send_event = 1;
            (*raw).focus_change.window = toplevel
                .window()
                .map(|w| w.to_glib_full())
                .unwrap_or(std::ptr::null_mut());
        }
        // The foreign GTK toplevel receives no native activation events from
        // winit. Let GTK update its toplevel state and propagate focus to WebKit.
        toplevel.send_focus_change(&event);
        if !self.focused {
            toplevel.set_focus(None::<&gtk::Widget>);
        }
        if let Some(native) = self.native
            && self.parent_active
        {
            // Transfer native keyboard focus as well as GTK's logical focus.
            // The widget's own window must receive complete IME key sequences.
            // Never focus the app if the user has switched to another window.
            if native
                .focused_window()
                .is_some_and(|id| id == native.window || id == self.child)
            {
                native.focus(if self.focused {
                    self.child
                } else {
                    native.window
                });
            }
        }
        log::debug!(target: "foam::webview", "embedded GTK focus={}", self.focused);
    }
}

pub(crate) struct Registration(u64);

impl Registration {
    pub fn new(
        webview: &WebView,
        parent: &Window,
        bounds: Rect,
        visible: bool,
    ) -> wry::Result<Self> {
        let widget: gtk::Widget = webview.webview().upcast();
        let toplevel = widget
            .toplevel()
            .and_then(|w| w.downcast::<gtk::Window>().ok())
            .ok_or_else(|| glib::bool_error!("Embedded WebKit has no GTK toplevel"))?;
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Xlib(parent_handle) = parent.window_handle()?.as_raw() else {
            return Err(wry::Error::UnsupportedWindowHandle);
        };
        let display = gtk::gdk::Display::default()
            .and_then(|d| d.downcast::<gdkx11::X11Display>().ok())
            .ok_or(wry::Error::X11DisplayNotFound)?;
        let native = NativeParent {
            display: unsafe { gdkx11::ffi::gdk_x11_display_get_xdisplay(display.as_ptr()) }
                as usize,
            window: parent_handle.window,
        };
        let native_child = widget
            .window()
            .and_then(|w| w.downcast::<gdkx11::X11Window>().ok())
            .ok_or_else(|| glib::bool_error!("Embedded WebKit has no native widget window"))?
            .xid();
        let key_masks = (
            widget
                .window()
                .map(|w| w.events())
                .unwrap_or_else(gtk::gdk::EventMask::empty),
            toplevel
                .window()
                .map(|w| w.events())
                .unwrap_or_else(gtk::gdk::EventMask::empty),
        );
        keyboard_events(&widget, &toplevel, key_masks, false);
        toplevel.set_focus(None::<&gtk::Widget>);
        let id = MANAGER.with(|slot| {
            let mut manager = slot.borrow_mut();
            manager.sequence += 1;
            let id = manager.sequence;
            manager
                .parents
                .entry(parent.id())
                .or_insert_with(|| Parent {
                    active: parent.has_focus(),
                    scale: parent.scale_factor(),
                    ..Parent::default()
                });
            manager.parents.get_mut(&parent.id()).unwrap().native = Some(native);
            manager.views.insert(
                id,
                View {
                    parent: parent.id(),
                    widget: widget.downgrade(),
                    toplevel: toplevel.downgrade(),
                    bounds,
                    visible,
                    focused: false,
                    order: id,
                    native_child,
                    key_masks,
                },
            );
            id
        });
        widget.connect_button_press_event(move |_, _| {
            change(|manager| {
                if let Some(view) = manager.views.get(&id) {
                    if !view.visible {
                        return;
                    }
                    let parent = view.parent;
                    // A real click requests activation, even if winit has not
                    // dispatched the corresponding Focused(true) yet.
                    manager.parents.get_mut(&parent).unwrap().active = true;
                    manager.select(parent, Some(id));
                }
            });
            glib::Propagation::Proceed
        });
        // Foreign toplevels can still receive key events while hidden or after
        // a Slint input was clicked. Stop those before GTK dispatches them.
        toplevel.connect_key_press_event(move |_, _| key_propagation(id));
        toplevel.connect_key_release_event(move |_, _| key_propagation(id));
        widget.connect_key_press_event(move |_, _| key_propagation(id));
        widget.connect_key_release_event(move |_, _| key_propagation(id));
        Ok(Self(id))
    }

    pub fn set_visible(&self, visible: bool) {
        change(|manager| {
            if visible {
                manager.sequence += 1;
                let order = manager.sequence;
                if let Some(view) = manager.views.get_mut(&self.0) {
                    view.visible = true;
                    view.order = order;
                }
            } else {
                manager.hide(self.0);
            }
        });
    }

    pub fn set_bounds(&self, bounds: Rect) {
        change(|manager| {
            if let Some(view) = manager.views.get_mut(&self.0) {
                view.bounds = bounds;
            }
        });
    }

    pub fn focus(&self) {
        change(|manager| {
            if let Some(view) = manager.views.get(&self.0) {
                if view.visible {
                    manager.select(view.parent, Some(self.0));
                }
            }
        });
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        change(|manager| manager.hide(self.0));
        let _ = MANAGER.try_with(|slot| {
            slot.borrow_mut().views.remove(&self.0);
        });
    }
}

fn key_propagation(id: u64) -> glib::Propagation {
    if MANAGER
        .try_with(|slot| slot.borrow().owns_keyboard(id))
        .unwrap_or(false)
    {
        glib::Propagation::Proceed
    } else {
        glib::Propagation::Stop
    }
}

/// Called before Slint processes input, including for windows created later.
pub(crate) fn window_event(window: &Window, event: &WindowEvent) -> EventResult {
    let parent_id = window.id();
    let effective_active = if matches!(event, WindowEvent::Focused(_)) {
        native_active(parent_id)
    } else {
        None
    };
    let mut consume = false;
    change(|manager| {
        let parent = manager.parents.entry(parent_id).or_insert_with(|| Parent {
            active: window.has_focus(),
            scale: window.scale_factor(),
            ..Parent::default()
        });
        match event {
            WindowEvent::Focused(active) => parent.active = effective_active.unwrap_or(*active),
            WindowEvent::CursorMoved { position, .. } => parent.cursor = Some(*position),
            WindowEvent::CursorLeft { .. } => parent.cursor = None,
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => parent.scale = *scale_factor,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                let hit = manager.hit(parent_id);
                manager.select(parent_id, hit);
                consume = hit.is_some();
            }
            WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_) => {
                consume = parent.selected.is_some_and(|id| manager.owns_keyboard(id));
            }
            WindowEvent::Destroyed => {
                manager.parents.remove(&parent_id);
            }
            _ => {}
        }
    });
    if consume {
        EventResult::PreventDefault
    } else {
        EventResult::Propagate
    }
}

pub(crate) struct ApplicationHandler;

impl i_slint_backend_winit::CustomApplicationHandler for ApplicationHandler {
    fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) -> EventResult {
        sync_native_focus();
        EventResult::Propagate
    }
    fn window_event(
        &mut self,
        _: &winit::event_loop::ActiveEventLoop,
        _: WindowId,
        window: Option<&Window>,
        _: Option<&slint::Window>,
        event: &WindowEvent,
    ) -> EventResult {
        window.map_or(EventResult::Propagate, |window| window_event(window, event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wry::dpi::{LogicalPosition, LogicalSize};

    fn fixture() -> (Manager, WindowId, WindowId) {
        let a = WindowId::from(1);
        let b = WindowId::from(2);
        let mut manager = Manager::default();
        for parent in [a, b] {
            manager.parents.insert(
                parent,
                Parent {
                    active: true,
                    scale: 2.,
                    ..Parent::default()
                },
            );
        }
        for (id, parent, visible) in [(1, a, true), (2, a, true), (3, a, false), (4, b, true)] {
            manager.views.insert(
                id,
                View {
                    parent,
                    visible,
                    focused: false,
                    order: id,
                    native_child: id as _,
                    key_masks: (gtk::gdk::EventMask::empty(), gtk::gdk::EventMask::empty()),
                    widget: glib::WeakRef::new(),
                    toplevel: glib::WeakRef::new(),
                    bounds: Rect {
                        position: LogicalPosition::new(10., 20.).into(),
                        size: LogicalSize::new(100., 80.).into(),
                    },
                },
            );
        }
        (manager, a, b)
    }

    #[test]
    fn focus_moves_exclusively_and_blurs_before_focusing() {
        let (mut manager, parent, _) = fixture();
        manager.select(parent, Some(2));
        assert_eq!(
            manager
                .changes()
                .iter()
                .map(|c| c.focused)
                .collect::<Vec<_>>(),
            [true]
        );
        manager.select(parent, Some(1));
        assert_eq!(
            manager
                .changes()
                .iter()
                .map(|c| c.focused)
                .collect::<Vec<_>>(),
            [false, true]
        );
        assert!(manager.owns_keyboard(1));
        assert!(!manager.owns_keyboard(2));
        assert!(manager.changes().is_empty());
        manager.select(parent, None); // Slint was clicked.
        assert_eq!(manager.changes().len(), 1);
        assert!(!manager.owns_keyboard(1));
    }

    #[test]
    fn application_switch_restores_selection_but_hide_clears_it() {
        let (mut manager, parent, other) = fixture();
        manager.select(parent, Some(1));
        manager.select(other, Some(4));
        manager.changes();
        manager.parents.get_mut(&parent).unwrap().active = false;
        assert_eq!(manager.changes().len(), 1);
        assert!(!manager.owns_keyboard(1));
        assert!(manager.owns_keyboard(4));
        manager.parents.get_mut(&parent).unwrap().active = true;
        assert_eq!(manager.changes().len(), 1);
        assert!(manager.owns_keyboard(1));
        manager.hide(1);
        assert_eq!(manager.changes().len(), 1);
        manager.views.get_mut(&1).unwrap().visible = true;
        assert!(manager.changes().is_empty());
        assert!(!manager.owns_keyboard(1));
    }

    #[test]
    fn hidden_recovery_cannot_take_keyboard_and_drop_after_parent_is_safe() {
        let (mut manager, parent, _) = fixture();
        manager.select(parent, Some(3));
        assert!(!manager.owns_keyboard(3));
        assert!(manager.changes().is_empty());
        manager.select(parent, Some(1));
        manager.changes();
        manager.parents.remove(&parent);
        assert_eq!(manager.changes().len(), 1);
        manager.hide(1); // Destruction may follow the parent's Destroyed event.
        manager.views.remove(&1);
        assert!(!manager.owns_keyboard(1));
    }

    #[test]
    fn registration_can_outlive_the_thread_local_manager() {
        // Match onboarding's TLS order: the slot exists before the first view
        // registers, so the manager can be destroyed before the stored view.
        thread_local! { static LAST_VIEW: RefCell<Option<Registration>> = const { RefCell::new(None) }; }
        std::thread::spawn(|| {
            LAST_VIEW.with(|slot| *slot.borrow_mut() = Some(Registration(42)));
            MANAGER.with(|slot| {
                let _ = slot.borrow();
            });
        })
        .join()
        .unwrap();
    }

    #[test]
    fn hit_testing_respects_scale_visibility_stacking_and_parent() {
        let (mut manager, parent, other) = fixture();
        manager.parents.get_mut(&parent).unwrap().cursor =
            Some(winit::dpi::PhysicalPosition::new(25., 45.));
        assert_eq!(manager.hit(parent), Some(2));
        manager.hide(2);
        assert_eq!(manager.hit(parent), Some(1));
        manager.parents.get_mut(&parent).unwrap().cursor =
            Some(winit::dpi::PhysicalPosition::new(220., 45.));
        assert_eq!(manager.hit(parent), None); // Right edge is outside.
        assert_eq!(manager.hit(other), None); // No cursor position for that parent.
    }

    #[test]
    fn activation_includes_visible_children_but_excludes_other_windows() {
        let (mut manager, parent, _) = fixture();
        manager.parents.get_mut(&parent).unwrap().native = Some(NativeParent {
            display: 0,
            window: 100,
        });
        assert!(manager.contains_native_focus(parent, 100));
        assert!(manager.contains_native_focus(parent, 1));
        assert!(!manager.contains_native_focus(parent, 3)); // Hidden recovery.
        assert!(!manager.contains_native_focus(parent, 4)); // Another parent.
        assert!(!manager.contains_native_focus(parent, 0)); // Outside the app.
    }
}
