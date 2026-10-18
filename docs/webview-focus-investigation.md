# Linux Schoology webview focus investigation

## Implemented embedded-panel fix

Foam retains its embedded panels. A shared `EmbeddedWebView` wrapper registers
onboarding, documents, attachments, and hidden session recovery with a Linux
focus manager. A click assigns keyboard ownership and supplies the missing
native focus event to the foreign GTK toplevel, then transfers X11 keyboard
focus to WebKit's own widget window. Both are necessary: logical GTK focus alone
fixes Microsoft's click behavior, but leaves fcitx composition incomplete. No
JavaScript focus overrides or keyboard-event replay are used.

Slint's backend hook releases GTK focus when a Slint control is clicked, blocks
duplicate Slint keyboard/IME delivery while WebKit owns input, and suspends or
restores selection when the application loses or gains focus. The existing GTK
pump cadence also samples native focus, because winit stops reporting parent
activation after keyboard focus enters a child. App titlebar activation follows
the whole parent/child window group. GTK key handlers
reject input for inactive or hidden views. Unfocused GTK windows also stop
selecting native key events, so moving the pointer over a panel does not divert
typing away from a Slint field. Hiding or dropping a view clears its
selection; showing it alone does not acquire keyboard focus. The wrapper also
avoids repeated native show/hide operations.

The narrow GTK GLX error filter is installed once during normal Linux startup.
It is enabled without environment flags. Other display connections and unrelated
X11 errors continue through winit's normal handling. Optional diagnostics remain
available without logging page text or credentials.

The standalone embedded probe and Foam both now report GTK focus-in and
`document_focused=true` on the same Microsoft sign-in page. Clicking its email
placeholder restores focus and accepts the first typed character. Local-page
checks cover Tab navigation, panel hide/show, window resizing/fullscreen,
repeated application switching, and clean process shutdown. The regression tests
also cover TLS destruction order, because views can outlive their focus registry.

Final GUI checks verified Microsoft email click-to-type with diagnostics disabled,
copy/paste in both directions between Slint and WebKit, typing into Slint while
hovering over WebKit, and fcitx pinyin committing `你好` in both toolkits. The
standalone lifecycle checks passed hidden-view creation/destruction without
stealing focus, key rejection while hidden, show without automatic focus, view
replacement, repeated application switching, and clean exit. These checks use
disposable profiles and dummy text. Account-backed document/attachment loading
and a completed authenticated onboarding/recovery still require a user sign-in.

## Original findings (2026-10-14, before the fix)

Both reported symptoms reproduce on `bins.schoology.com`, which redirects to
Microsoft sign-in. There are two integration problems:

1. The embedded WebKit widget never receives GTK focus-in. Its `is_focus` is
   true, but `has_focus` and the page's `document.hasFocus()` remain false.
   Clicking Microsoft's email placeholder blurs the input and does not restore
   it. Tab can select the input and allow typing, despite the missing document
   focus. This is a mouse/focus handoff failure, not a total keyboard failure.
2. GTK's X11 connection emits `GLXBadWindow` for `GLXDestroyWindow` immediately
   after constructing the child webview, before clicking the input. Winit's
   process-wide Xlib handler stores that error, including errors from GTK's
   separate display connection. Switching to the terminal makes winit call
   `XUnsetICFocus`, then check that stored error and panic. The panic location
   does not identify the operation that produced the GLX error.

The opt-in error workaround prevents the panic but leaves the focus failure.
An ordinary GTK-owned window hosting Wry receives GTK and DOM focus normally,
including on the same Microsoft page. No authenticated sign-in was attempted.

Environment: Slint/backend 1.18.1, winit 0.30.13, Wry 0.57.0, GTK 3.24.52,
WebKitGTK 2.54.1, Sway 1.12 through XWayland; `GTK_IM_MODULE=fcitx` and
`XMODIFIERS=@im=fcitx`. Dependency versions were held fixed.

## Evidence and controlled comparisons

The original comparison runs clicked the input/placeholder, typed a dummy character, pressed Tab,
typed another dummy character, and switched to the terminal. Local probes used
ordinary email/password inputs. GUI input went through Sway's cursor and X11
keyboard events; DOM focus was observed rather than forced with JavaScript.

| Host/page | Change from baseline | Focus result | Panic on switching away |
| --- | --- | --- | --- |
| Unmodified Foam / Microsoft | None | Email click fails | Exact reported panic |
| Instrumented Foam / Microsoft | Observation only | Placeholder click blurs input; document focus false | Yes |
| Foam / Microsoft | Narrow GLX error hook | Same focus failure; Tab permits typing | No |
| Foam / Microsoft | Slint software renderer | Same focus failure | Yes |
| Foam / Microsoft | Unset fcitx environment variables | Same focus failure | Yes |
| Foam / local inputs | Observation only | Click/Tab accept text, but document focus stays false | Yes |
| Foam / local inputs | Narrow GLX error hook | Same missing document focus | No |
| Foam / local inputs | Slint software renderer | Same missing document focus | Yes |
| Foam / local inputs | Unset fcitx environment variables | Same missing document focus | Yes |
| Standalone winit + Wry / local inputs | Removes Slint, accounts, onboarding | Same missing GTK/DOM focus | Yes |
| Standalone winit + Wry / local inputs | Narrow GLX error hook | Same missing GTK/DOM focus | No |
| GTK + Wry / local inputs | GTK owns the host window | GTK focus-in; document focus true; typing works | No |
| GTK + Wry / Microsoft | Same Schoology redirect | Placeholder click restores input focus; typing works | No |

Unsetting the fcitx variables is an environment comparison, not proof that every
system input-method configuration was disabled. Changing the Slint renderer
does not change GTK/WebKit's graphics implementation.

Representative trace, in order:

```text
X11 diagnostics: gtk_display=0x556192469aa0 glx=true opcode=150 error_base=158 workaround=false
onboarding update: step=2 visible=true ... exists=false
X11 error: display=0x556192469aa0 resource=0x1c0000a serial=311 code=170 request=150 minor=32 handled=false
DOM event=Ready target=Body active=Body document_focused=false top_frame=true
GTK button-press has_focus=false is_focus=true
```

Microsoft's embedded page then reports `Pointerdown target=Other active=Input`,
followed by `Focusout target=Input active=Body`, without restoring focus. The
GTK-owned control reports the same initial click followed by `Focusin` and
`Input`, with `document_focused=true`.

A native debugger breakpoint on `glXDestroyWindow` in the standalone winit
probe showed this call path (system libraries lack full debugging symbols):

```text
glXDestroyWindow (libGL)
  <- libgdk-3
  <- g_datalist_clear / g_object_unref
  <- libgdk-3
  <- gdk_display_get_event
  <- g_main_context_iteration
  <- gtk_main_iteration_do
  <- webview_focus::Probe::about_to_wait
```

The error arrives while GTK processes its events. It is not generated by
Schoology JavaScript or winit's later IME unfocus operation. No onboarding
visibility/bounds update occurred between the settled page and the failing
click. The standalone reproduction also removes Foam's clipboard adapter and
Slint layout from the causal path.

An additional local-page run with the workaround survived resize, fullscreen
toggle, Previous/Next hide/show, and repeated terminal/app switching. DOM focus
still stayed false. A copy/paste handoff attempt also produced WebKit key events
after hiding the panel; successful clipboard transfer into Slint was not
established. The workaround therefore does not satisfy focus or input-routing
acceptance criteria.

## Running the diagnostics

Build the app and both standalone host modes:

```sh
cargo build --locked --bin foam --example webview_focus
```

Use a fresh profile and enter `bins` during onboarding. Existing account data
and cookies are preserved:

```sh
probe_data=$(mktemp -d /tmp/foam-webview.XXXXXX)
env XDG_DATA_HOME="$probe_data" RUST_BACKTRACE=full \
  RUST_LOG=foam=info FOAM_WEBVIEW_DIAGNOSTICS=1 target/debug/foam
```

The diagnostics log native parent focus, display/window identities, GTK widget
focus, DOM focus/key/input event categories, onboarding lifecycle updates, and
X11 error metadata. DOM messages are parsed into a fixed vocabulary; arbitrary
page text, values, keys, URLs, and credentials are not logged by this bridge.
Diagnostics are disabled unless explicitly enabled.

For the local page, also set `FOAM_WEBVIEW_TEST_PAGE=1`. This replaces the initial
onboarding browser document with the local probe. It cannot complete account
verification; it is for input testing only.

The crash mitigation is enabled in Foam by default. The standalone probe can
disable it with `FOAM_X11_ERROR_WORKAROUND=0` to observe the original GLX panic.
The hook handles only GTK's display connection,
the server's GLX opcode, GLXBadWindow (`error_base + 12`), and DestroyWindow
(`minor_code=32`). Other X11 errors remain visible to winit. Error numbers are
queried from the server rather than hardcoded to `170`/`150`.

Standalone local page comparisons, independent of Foam's account state:

```sh
env RUST_BACKTRACE=full FOAM_WEBVIEW_DIAGNOSTICS=1 \
  target/debug/examples/webview_focus winit
env RUST_BACKTRACE=full FOAM_WEBVIEW_DIAGNOSTICS=1 \
  target/debug/examples/webview_focus gtk
```

Pass a URL after the host to compare the actual sign-in page:

```sh
env FOAM_WEBVIEW_DIAGNOSTICS=1 \
  target/debug/examples/webview_focus gtk https://bins.schoology.com
```

For each comparison, change one variable: add `SLINT_BACKEND=winit-software`
for Foam, disable the standalone probe's crash mitigation, or prefix `env` with
`-u GTK_IM_MODULE -u XMODIFIERS`. Inspect click focus separately from text input
after Tab and separately from the panic. Close one GUI before starting another.

## Implementation and validation

`src/embedded_webview.rs` centralizes creation, visibility, geometry, and
registration lifetime. The Linux focus manager records the parent, bounds,
stacking order, visibility, and selected view. It emits GTK callbacks only after
releasing its registry borrow and uses weak widget references. Cleanup tolerates
thread-local destruction order and parent destruction before child registration.

The initial GTK-owned-window recommendation is superseded by the verified native
focus bridge, which preserves the existing embedded layout and Schoology
`WebContext`. Dependency versions and authentication/cookie logic are unchanged.

The standalone `slint` host places an editable Slint field beside the native
panel, using the production Linux backend and clipboard implementation:

```sh
env FOAM_WEBVIEW_DIAGNOSTICS=1 target/debug/examples/webview_focus slint
```

The standalone winit probe accepts commands on stdin while its GTK event loop
continues running:

- `hide` / `show` hide or show the current panel.
- `replace` destroys and replaces it.
- `background` toggles a hidden background view, simulating silent recovery.

These commands work independently of which toolkit currently owns the keyboard.

Build and run the focused regression tests:

```sh
cargo build --locked --offline --bin foam --example webview_focus
cargo test --locked --offline --example webview_focus
node --check src/platform/linux/webview_focus.js
```

Eight focused tests cover focus ownership, blur-before-focus ordering, reactivation, hidden
views, scaled/overlapping bounds, separate parents, parent/TLS destruction,
dynamic GLX numbering, unrelated display connections/errors, and rejection of
DOM messages containing input values or arbitrary text. Existing
unrelated Rust dead-code/import warnings and a Slint `viewport-y` deprecation
remain.

Acceptance checks include actual click-to-type and
visible caret on Microsoft and local inputs, Tab/Shift-Tab, copy/paste between
Slint and WebKit, repeated application/terminal switching, resizing/maximizing,
and Previous/Next webview visibility transitions. A successful authenticated
onboarding and session recovery still require the user's credentials; those
were not exercised automatically.

Upstream references:

- [Wry 0.57.0 winit example](https://github.com/tauri-apps/wry/blob/wry-v0.57.0/examples/winit.rs#L89)
  handles error code 170 without the narrower display/request checks.
- [Wry 0.57.0 Linux embedding implementation](https://github.com/tauri-apps/wry/blob/wry-v0.57.0/src/webkitgtk/mod.rs)
  wraps an X11 foreign window in a GTK toplevel; `focus()` calls `grab_focus()`.
- [winit 0.30.13 Xlib handler](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/mod.rs)
  stores unhandled errors in the winit connection's `latest_error`.
