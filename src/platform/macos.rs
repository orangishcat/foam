/**
 * this entire file exists because macos cmd+q will instantly terminate the program,
 * skipping shutdown, and this file will make it... not do that
 */

#[cfg(target_os = "macos")]
pub mod macos_quit {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
    use objc2_app_kit::{NSApplication, NSMenu};
    use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol};

    declare_class!(
        pub struct QuitTarget;

        unsafe impl ClassType for QuitTarget {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "FoamQuitTarget";
        }

        impl DeclaredClass for QuitTarget {}
        unsafe impl NSObjectProtocol for QuitTarget {}

        unsafe impl QuitTarget {
            #[method(foamQuit:)]
            fn quit(&self, _sender: Option<&AnyObject>) {
                if let Err(error) = slint::quit_event_loop() {
                    log::error!("Could not request shutdown: {error}");
                }
            }
        }
    );

    fn redirect_quit(menu: &NSMenu, target: &QuitTarget) -> bool {
        // AppKit objects are accessed on the main thread. The caller retains
        // target throughout the event loop because menu item targets are weak.
        unsafe {
            for item in menu.itemArray() {
                if matches!(item.action(), Some(action) if action == sel!(terminate:) || action == sel!(foamQuit:))
                {
                    item.setTarget(Some(target));
                    item.setAction(Some(sel!(foamQuit:)));
                    return true;
                }
                if let Some(submenu) = item.submenu()
                    && redirect_quit(&submenu, target)
                {
                    return true;
                }
            }
        }
        false
    }

    pub fn new_target() -> Retained<QuitTarget> {
        let mtm = MainThreadMarker::new().expect("must run on the main thread");
        unsafe { msg_send_id![super(mtm.alloc().set_ivars(())), init] }
    }

    pub fn on_focus(target: &Retained<QuitTarget>) {
        let mtm = MainThreadMarker::new().expect("must run on the main thread");
        let callback_target = target.clone();
        // Slint creates/activates its Muda menu while processing window focus.
        // The window-event hook runs before that processing, so defer until
        // the event has propagated. Repeat on focus to handle menu replacement.
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let app = NSApplication::sharedApplication(mtm);
            let installed = unsafe { app.mainMenu() }
                .is_some_and(|menu| redirect_quit(&menu, &callback_target));
            if !installed {
                log::error!("Could not find the native macOS Quit menu item");
            }
        });
    }
}
