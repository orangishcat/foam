/**
 * this entire file exists because macos cmd+q will instantly terminate the program,
 * skipping shutdown, and this file will make it... not do that
 */

#[cfg(target_os = "macos")]
pub mod macos_quit {
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability};
    use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSApplicationTerminateReply};
    use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol};

    declare_class!(
        pub struct QuitDelegate;

        unsafe impl ClassType for QuitDelegate {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "FoamQuitDelegate";
        }

        impl DeclaredClass for QuitDelegate {}
        unsafe impl NSObjectProtocol for QuitDelegate {}

        unsafe impl NSApplicationDelegate for QuitDelegate {
            #[method(applicationShouldTerminate:)]
            fn should_terminate(&self, _app: &NSApplication) -> NSApplicationTerminateReply {
                if let Err(error) = slint::quit_event_loop() {
                    log::error!("Could not request shutdown: {error}");
                }
                NSApplicationTerminateReply::NSTerminateCancel
            }
        }
    );

    pub fn install() -> Retained<QuitDelegate> {
        let mtm = MainThreadMarker::new().expect("must run on the main thread");
        let delegate: Retained<QuitDelegate> =
            unsafe { msg_send_id![super(mtm.alloc().set_ivars(())), init] };
        NSApplication::sharedApplication(mtm)
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        delegate
    }
}
