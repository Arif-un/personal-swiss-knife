//! Builds (or shows) the Messenger window and applies the native-layer corner
//! rounding. Holds the injected JS/CSS assets that drive the in-page overlays.

use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::messenger::{MESSENGER_LABEL, MESSENGER_URL};

/// Desktop Safari user agent. WKWebView's default UA gets flagged as suspicious
/// by Facebook far more often (extra captcha / checkpoints during login), so we
/// present as a normal desktop browser.
pub(super) const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";

/// Injected into the Messenger webview before every page load. Intercepts anchor
/// clicks and hands the shim-unwrapped target plus the held modifier flags to
/// Rust via the custom `swissknife-link://` scheme, so `on_navigation` can apply
/// the user's routing rules (`bubble::LinkRules`). Facebook links are captured
/// except Messenger's own app routes under `/messages` (thread switching, the
/// rail), which are left to the SPA so in-app navigation keeps working. Every
/// other Facebook link (posts, reels, profiles) and all non-Facebook links are
/// captured. Logins, redirects, and captchas aren't anchor clicks, so they're
/// untouched too.
const INTERCEPTOR_JS: &str = include_str!("js/interceptor.js");

/// Injected into the Messenger webview to draw a floating, macOS-style window
/// control pill (close / minimize / zoom) in the top-left corner. The window is
/// built with `decorations(false)`, so it has no native title bar or traffic
/// lights — this replaces them.
///
/// Button clicks signal Rust through the same `swissknife-link://` scheme the
/// link interceptor uses (host `window`, `action` query), so the remote page
/// still gets no IPC/ACL access to real commands. Dragging the pill's empty area
/// uses `data-tauri-drag-region`, which needs `core:window:allow-start-dragging`
/// granted to this window (see `capabilities/messenger.json`).
///
/// The pill and drag strip are re-added on an interval because Facebook is an
/// SPA that can wipe the DOM out from under us on navigation.
const TITLEBAR_JS: &str = include_str!("js/titlebar.js");

/// Injected into the Messenger webview to draw the floating "chat-head" bubble
/// overlay used when the window is collapsed. State is pushed from Rust via
/// `eval` and actions come back through the same `swissknife-link://` scheme.
/// See `bubble.rs` for the window-morphing logic.
const BUBBLE_JS: &str = include_str!("js/bubble.js");

/// Injected into the Messenger webview to draw the modal frame (dim backdrop +
/// header/close bar) around the link-preview child webview. The preview content
/// is a native child webview attached to the Messenger window (see `open_peek`);
/// this only draws the chrome around it. See `js/peek.js`.
const PEEK_JS: &str = include_str!("js/peek.js");

/// The overlay stylesheet, injected once via `inject-style.js`. Tauri's only
/// page-injection channel here is `initialization_script` (JS, not raw CSS), so
/// the CSS is handed to the page as a JS global and `inject-style.js` mounts it
/// as a `<style>` element (re-adding it after the SPA wipes the DOM).
const MESSENGER_CSS: &str = include_str!("css/messenger.css");
const INJECT_STYLE_JS: &str = include_str!("js/inject-style.js");

/// Round the corners of a decorationless, transparent macOS window. CSS
/// `border-radius` can't do this reliably here: the Messenger page uses
/// `position:fixed` full-viewport layers that escape any ancestor's overflow
/// clip and repaint square corners. Masking the native content view's layer
/// clips the whole webview at the compositing level, immune to page CSS.
#[cfg(target_os = "macos")]
fn round_corners(win: &WebviewWindow, radius: f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let Ok(ns_window) = win.ns_window() else {
        return;
    };
    let ns_window = ns_window as *mut AnyObject;
    unsafe {
        let content_view: *mut AnyObject = msg_send![ns_window, contentView];
        let _: () = msg_send![content_view, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![content_view, layer];
        let _: () = msg_send![layer, setCornerRadius: radius];
        let _: () = msg_send![layer, setMasksToBounds: true];
    }
}

#[cfg(not(target_os = "macos"))]
fn round_corners(_win: &WebviewWindow, _radius: f64) {}

/// Round + clip a single `WKWebView`'s own layer. With multiwebview the page
/// renders in its own `WKWebView` subview, which the window content-view mask in
/// `round_corners` does NOT clip — so Facebook's square corners bleed past the
/// rounded window border (and the child preview past its frame). Masking the
/// webview's layer clips the page to the view's bounds at the compositing level.
/// `corners` is a `CACornerMask` bitfield; `0` leaves the default (all four).
/// macOS layer coords are bottom-left origin: bottom corners = `1 | 2 = 3`.
#[cfg(target_os = "macos")]
pub(super) fn mask_webview(view_ptr: *mut objc2::runtime::AnyObject, radius: f64, corners: u64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    unsafe {
        let _: () = msg_send![view_ptr, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![view_ptr, layer];
        let _: () = msg_send![layer, setCornerRadius: radius];
        if corners != 0 {
            let _: () = msg_send![layer, setMaskedCorners: corners];
        }
        let _: () = msg_send![layer, setMasksToBounds: true];
    }
}

/// Build the Messenger window (or show + focus it if already warm). Trusted Rust
/// entry point shared by the `messenger_open` command and the global toggle
/// shortcut, so it skips the `require_main` gate the command applies.
pub fn open_or_show(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        crate::messenger::bubble::on_opened(app);
        return Ok(());
    }
    let url: Url = MESSENGER_URL
        .parse()
        .map_err(|_| "invalid messenger url".to_string())?;
    let handle = app.clone();
    let win = WebviewWindowBuilder::new(app, MESSENGER_LABEL, WebviewUrl::External(url))
        .title("Messenger")
        .inner_size(1000.0, 760.0)
        .decorations(false)
        .transparent(true)
        // Deliver the first click on the unfocused bubble straight to the webview so a
        // single click expands (macOS otherwise eats the activating mousedown).
        .accept_first_mouse(true)
        .user_agent(DESKTOP_UA)
        // CSS first so the stylesheet exists before the overlay JS builds elements.
        .initialization_script(format!(
            "window.__SK_CSS={};",
            serde_json::to_string(MESSENGER_CSS).unwrap_or_default()
        ))
        .initialization_script(INJECT_STYLE_JS)
        .initialization_script(INTERCEPTOR_JS)
        .initialization_script(TITLEBAR_JS)
        .initialization_script(BUBBLE_JS)
        .initialization_script(PEEK_JS)
        .on_navigation(move |url| super::navigate::on_navigation(&handle, url))
        .build()
        .map_err(|e| e.to_string())?;
    // Restore the user's last full-window size + position (a cold launch builds a
    // fresh window at the default inner_size otherwise).
    crate::messenger::bubble::apply_saved_full_geometry(app, &win);
    round_corners(&win, 12.0);
    // Clip the page's own WKWebView to the rounded window (the content-view mask
    // above doesn't reach the webview subview, so FB spills past the corners).
    #[cfg(target_os = "macos")]
    {
        let _ = win.with_webview(|pw| {
            mask_webview(pw.inner() as *mut objc2::runtime::AnyObject, 12.0, 0);
        });
    }

    // Route native context-menu selections (from the bubble's right-click menu)
    // back into the bubble module.
    let menu_handle = app.clone();
    win.on_menu_event(move |_win, event| {
        crate::messenger::bubble::on_menu_event(&menu_handle, event.id().as_ref());
    });
    // A fresh window is always built full-sized, so sync the tracked mode. Skipping
    // this leaves `mode` stale at Bubble after a quit-while-collapsed + rebuild, so
    // the next toggle mis-routes to enter_full and auto-collapse stays disarmed.
    crate::messenger::bubble::mark_full(app);
    crate::messenger::bubble::on_opened(app);
    Ok(())
}
