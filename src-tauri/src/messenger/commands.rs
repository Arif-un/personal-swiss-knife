use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Url, WebviewBuilder, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;

use crate::messenger::{MESSENGER_LABEL, MESSENGER_URL, PEEK_LABEL, ROUTE_SCHEME};

/// Desktop Safari user agent. WKWebView's default UA gets flagged as suspicious
/// by Facebook far more often (extra captcha / checkpoints during login), so we
/// present as a normal desktop browser.
const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";

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

/// Injected into the preview child webview so Esc closes it even when the panel
/// (not the Messenger page) has keyboard focus. Routes through the shared scheme,
/// caught by the child's own `on_navigation` -> `close_peek`.
const PEEK_ESC_JS: &str = "document.addEventListener('keydown',function(e){if(e.key==='Escape'){window.location.href='swissknife-link://window?action=peek-close';}},true);";

/// Inset of the link-preview modal from the Messenger window edges, and the
/// height of its header bar. The preview child webview is placed in the rect
/// below the header; `peek.js` / `messenger.css` draw the frame around it, so
/// these MUST stay in sync with the geometry in `messenger.css`.
const PEEK_MARGIN: f64 = 24.0;
const PEEK_HEADER: f64 = 40.0;

/// The overlay stylesheet, injected once via `inject-style.js`. Tauri's only
/// page-injection channel here is `initialization_script` (JS, not raw CSS), so
/// the CSS is handed to the page as a JS global and `inject-style.js` mounts it
/// as a `<style>` element (re-adding it after the SPA wipes the DOM).
const MESSENGER_CSS: &str = include_str!("css/messenger.css");
const INJECT_STYLE_JS: &str = include_str!("js/inject-style.js");

/// Whether `raw` parses as an `http`/`https` URL. The click interceptor's own
/// scheme check runs inside the remote, untrusted Messenger page and is trivially
/// bypassable (a script can navigate straight to `swissknife-link://...`), so we
/// re-check the scheme here in Rust before handing anything to the OS opener or a
/// preview window. This refuses `file://` and arbitrary custom schemes, which
/// would otherwise let untrusted page content open local files or trigger
/// registered protocol handlers.
fn is_web_url(raw: &str) -> bool {
    Url::parse(raw)
        .map(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// Open a URL in the user's default system browser.
fn open_external(app: &AppHandle, url: &str) {
    let _ = app.opener().open_url(url.to_string(), None::<&str>);
}

/// Logical (position, size) for the preview child webview: inset by `PEEK_MARGIN`
/// on the sides and bottom, and sitting below the `PEEK_HEADER` bar at the top.
fn peek_bounds(win: &WebviewWindow) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
    let scale = win.scale_factor().ok()?;
    let sz = win.inner_size().ok()?;
    let w = sz.width as f64 / scale;
    let h = sz.height as f64 / scale;
    let y = PEEK_MARGIN + PEEK_HEADER;
    let cw = (w - 2.0 * PEEK_MARGIN).max(120.0);
    let ch = (h - y - PEEK_MARGIN).max(120.0);
    Some((
        LogicalPosition::new(PEEK_MARGIN, y),
        LogicalSize::new(cw, ch),
    ))
}

/// Open a URL in the link-preview panel: a native child webview overlaid on the
/// Messenger window (an in-window modal, not a separate OS window). A native
/// webview loads sites that refuse iframing (Facebook, banks, ...), which an
/// in-page iframe modal can't. Reuses the open preview if there is one (navigate
/// and resize), else attaches a new child. `peek.js` draws the modal frame
/// backdrop and header around it in the Messenger page.
fn open_peek(app: &AppHandle, url: &str) {
    let target: Url = match url.parse() {
        Ok(u) => u,
        Err(_) => return,
    };
    let Some(mwin) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    let Some((pos, size)) = peek_bounds(&mwin) else {
        return;
    };
    // Reuse an already-open preview: navigate to the new target.
    if let Some(wv) = app.get_webview(PEEK_LABEL) {
        let _ = wv.navigate(target);
        let _ = wv.set_focus();
    } else {
        // Attach a fresh child webview to the Messenger window (multiwebview; needs
        // the tauri `unstable` feature). `peek_bounds` is only a seed size; peek.js
        // reports the exact CSS-px rect once shown (see the `peek` host above).
        // Only our close scheme is intercepted; every other link navigates within
        // the preview.
        let Some(win) = app.get_window(MESSENGER_LABEL) else {
            return;
        };
        let esc_handle = app.clone();
        let child = win.add_child(
            WebviewBuilder::new(PEEK_LABEL, WebviewUrl::External(target))
                .user_agent(DESKTOP_UA)
                .initialization_script(PEEK_ESC_JS)
                .on_navigation(move |u| {
                    if u.scheme() == ROUTE_SCHEME {
                        close_peek(&esc_handle);
                        return false;
                    }
                    true
                }),
            pos,
            size,
        );
        // Clip FB to the child's frame so it can't paint past the modal. All four
        // corners rounded (a card under the header); masksToBounds does the clip.
        #[cfg(target_os = "macos")]
        if let Ok(child) = child {
            let _ = child.with_webview(|pw| {
                mask_webview(pw.inner() as *mut objc2::runtime::AnyObject, 10.0, 0);
            });
        }
        #[cfg(not(target_os = "macos"))]
        let _ = child;
    }
    // Draw the modal frame (dim backdrop + header) in the Messenger page and let
    // it report the exact child rect. Done after the child exists so the report
    // lands on a live webview.
    let _ = mwin.eval(format!(
        "window.__skPeekShow&&window.__skPeekShow({})",
        serde_json::to_string(url).unwrap_or_default()
    ));
}

/// Close the link-preview panel: destroy the child webview and hide its modal
/// frame. Reached from the frame's close button / backdrop (`peek-close`) and
/// when the window collapses to a bubble.
pub fn close_peek(app: &AppHandle) {
    if let Some(wv) = app.get_webview(PEEK_LABEL) {
        let _ = wv.close();
    }
    if let Some(mwin) = app.get_webview_window(MESSENGER_LABEL) {
        let _ = mwin.eval("window.__skPeekHide&&window.__skPeekHide()");
    }
}

/// Apply a window control action signalled by the injected floating title bar.
/// `close` hides (keeps the window warm, matching the native close handler in
/// `lib.rs`); use `messenger_close` to actually reclaim RAM.
fn window_action(app: &AppHandle, action: &str) {
    use crate::messenger::bubble;

    // Bubble actions manage the window themselves (resize/destroy/menu), so they
    // don't need the window fetched up front.
    match action {
        "collapse" => return bubble::enter_bubble(app, true),
        "expand" => return bubble::expand_click(app),
        "hide" => return bubble::hide(app),
        "quit" => return bubble::quit(app),
        "menu" => return bubble::show_menu(app),
        "peek-close" => return close_peek(app),
        _ => {}
    }

    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    match action {
        "close" => {
            let _ = win.hide();
        }
        "minimize" => {
            let _ = win.minimize();
        }
        "zoom" => {
            if matches!(win.is_maximized(), Ok(true)) {
                let _ = win.unmaximize();
            } else {
                let _ = win.maximize();
            }
        }
        _ => {}
    }
}

/// `on_navigation` handler for the Messenger window. Returns `true` to let the
/// navigation proceed inside the webview, `false` to cancel it.
///
/// We only ever intercept the interceptor's own routing signal (the custom
/// scheme). Every other navigation — including login redirects, checkpoints, and
/// captcha challenges (which can live on non-facebook.com domains) — is allowed
/// to proceed in this window so it keeps the same session and cookies. Routing a
/// captcha out to the separate preview window would solve it in a different
/// session, so login would never complete.
fn on_navigation(app: &AppHandle, url: &Url) -> bool {
    if url.scheme() != ROUTE_SCHEME {
        return true;
    }

    // Floating title bar control (close / minimize / zoom): apply, then cancel.
    if url.host_str() == Some("window") {
        let action = url
            .query_pairs()
            .find(|(k, _)| k == "action")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        window_action(app, &action);
        return false;
    }

    // Geometry report from peek.js (CSS px, authoritative): size the child to it.
    // The page measures the modal hole in its own CSS pixels, which is the only
    // frame that always lines up with the drawn backdrop/header.
    if url.host_str() == Some("peek") {
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 0.0, 0.0);
        for (k, v) in url.query_pairs() {
            let n = v.parse::<f64>().unwrap_or(0.0);
            match k.as_ref() {
                "x" => x = n,
                "y" => y = n,
                "w" => w = n,
                "h" => h = n,
                _ => {}
            }
        }
        if w >= 1.0 && h >= 1.0 {
            if let Some(wv) = app.get_webview(PEEK_LABEL) {
                let _ = wv.set_position(LogicalPosition::new(x, y));
                let _ = wv.set_size(LogicalSize::new(w, h));
            }
        }
        return false;
    }

    // Intent signalled by the injected interceptor: `fb` marks a Facebook-family
    // destination, the modifier flags carry the held keys, `url` is the
    // shim-unwrapped target. Resolve to an action via the user's rules, then
    // cancel so the custom scheme never actually loads.
    let mut fb = false;
    let (mut meta, mut ctrl, mut alt, mut shift) = (false, false, false, false);
    let mut target = String::new();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "fb" => fb = v == "1",
            "meta" => meta = v == "1",
            "ctrl" => ctrl = v == "1",
            "alt" => alt = v == "1",
            "shift" => shift = v == "1",
            "url" => target = v.into_owned(),
            _ => {}
        }
    }
    if is_web_url(&target) {
        let action =
            crate::messenger::bubble::get_link_rules(app).resolve(fb, meta, ctrl, alt, shift);
        route_link(app, action, &target);
    }
    false
}

/// Apply a resolved link action to a target URL.
fn route_link(app: &AppHandle, action: crate::messenger::bubble::LinkAction, target: &str) {
    use crate::messenger::bubble::LinkAction;
    match action {
        LinkAction::SystemBrowser => open_external(app, target),
        LinkAction::ChildWebview => open_peek(app, target),
        LinkAction::SameWindow => {
            if let (Some(win), Ok(u)) = (
                app.get_webview_window(MESSENGER_LABEL),
                target.parse::<Url>(),
            ) {
                let _ = win.navigate(u);
            }
        }
    }
}

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
fn mask_webview(view_ptr: *mut objc2::runtime::AnyObject, radius: f64, corners: u64) {
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

/// Open the Messenger window, or show + focus it if it already exists. The window
/// is created lazily on first use and, thanks to the close handler in `lib.rs`,
/// is hidden (not destroyed) on close so reopening is instant.
#[tauri::command]
pub fn messenger_open(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    crate::security::require_main(&window)?;
    open_or_show(&app)
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
        .on_navigation(move |url| on_navigation(&handle, url))
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

/// Read the current global toggle shortcut (a Tauri accelerator string) so the
/// Messenger settings page can show and record it.
#[tauri::command]
pub fn messenger_get_shortcut(window: WebviewWindow, app: AppHandle) -> Result<String, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_shortcut(&app))
}

/// Rebind the global toggle shortcut. Registers the new accelerator (replacing the
/// old) and persists it. Errors on an unparseable/occupied accelerator.
#[tauri::command]
pub fn messenger_set_shortcut(
    window: WebviewWindow,
    app: AppHandle,
    accelerator: String,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_shortcut(&app, &accelerator)
}

/// Read the current auto-collapse idle timeout (seconds; 0 = disabled) so the
/// Messenger settings page can show it.
#[tauri::command]
pub fn messenger_get_idle_secs(window: WebviewWindow, app: AppHandle) -> Result<u64, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_idle_secs(&app))
}

/// Set the auto-collapse idle timeout (seconds; 0 = disabled) and persist it.
#[tauri::command]
pub fn messenger_set_idle_secs(
    window: WebviewWindow,
    app: AppHandle,
    secs: u64,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_idle_secs(&app, secs);
    Ok(())
}

/// Read whether the unread badge is muted so the settings page can show it.
#[tauri::command]
pub fn messenger_get_muted(window: WebviewWindow, app: AppHandle) -> Result<bool, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_muted(&app))
}

/// Mute or unmute the unread badge and persist it.
#[tauri::command]
pub fn messenger_set_muted(
    window: WebviewWindow,
    app: AppHandle,
    muted: bool,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_muted(&app, muted);
    Ok(())
}

/// Read the link-routing rules so the settings page can show and edit them.
#[tauri::command]
pub fn messenger_get_link_rules(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<crate::messenger::bubble::LinkRules, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_link_rules(&app))
}

/// Replace the link-routing rules from the settings page and persist them.
#[tauri::command]
pub fn messenger_set_link_rules(
    window: WebviewWindow,
    app: AppHandle,
    rules: crate::messenger::bubble::LinkRules,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_link_rules(&app, rules);
    Ok(())
}

/// Destroy the Messenger window to reclaim its RAM (as opposed to the default
/// close, which only hides it). The preview child webview is destroyed with its
/// parent window, but close it first so the frame state is cleared too.
#[tauri::command]
pub fn messenger_close(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    crate::security::require_main(&window)?;
    close_peek(&app);
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}
