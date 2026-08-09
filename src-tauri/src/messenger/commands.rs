use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

use crate::messenger::{MESSENGER_LABEL, MESSENGER_URL, PEEK_LABEL, ROUTE_SCHEME};

/// Desktop Safari user agent. WKWebView's default UA gets flagged as suspicious
/// by Facebook far more often (extra captcha / checkpoints during login), so we
/// present as a normal desktop browser.
const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";

/// Injected into the Messenger webview before every page load. Intercepts user
/// clicks on external links and hands the target to Rust via the custom
/// `swissknife-link://` scheme so `on_navigation` can route it. Internal Facebook
/// navigation (and anything not an anchor click — logins, redirects, captchas) is
/// left untouched so switching chats and signing in work normally.
///
/// Shift-click routes to the system browser; a plain click routes to the reusable
/// preview ("peek") window.
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

/// Open a URL in the reusable preview window: reuse the existing "peek" window if
/// present (navigate + focus), otherwise create a bare one. Closing that window
/// destroys it, so its RAM is reclaimed rather than kept warm.
fn open_peek(app: &AppHandle, url: &str) {
    let target: Url = match url.parse() {
        Ok(u) => u,
        Err(_) => return,
    };
    if let Some(win) = app.get_webview_window(PEEK_LABEL) {
        let _ = win.navigate(target);
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, PEEK_LABEL, WebviewUrl::External(target))
        .title("Preview")
        .inner_size(1000.0, 720.0)
        .build();
}

/// Apply a window control action signalled by the injected floating title bar.
/// `close` hides (keeps the window warm, matching the native close handler in
/// `lib.rs`); use `messenger_close` to actually reclaim RAM.
fn window_action(app: &AppHandle, action: &str) {
    use crate::messenger::bubble;

    // Bubble actions manage the window themselves (resize/destroy/menu), so they
    // don't need the window fetched up front.
    match action {
        "collapse" => return bubble::enter_bubble(app),
        "expand" => return bubble::expand_click(app),
        "hide" => return bubble::hide(app),
        "quit" => return bubble::quit(app),
        "menu" => return bubble::show_menu(app),
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

    // Intent signalled by the injected interceptor: route, then cancel so the
    // custom scheme never actually loads.
    let mut mode = String::from("peek");
    let mut target = String::new();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "mode" => mode = v.into_owned(),
            "url" => target = v.into_owned(),
            _ => {}
        }
    }
    if is_web_url(&target) {
        if mode == "browser" {
            open_external(app, &target);
        } else {
            open_peek(app, &target);
        }
    }
    false
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

/// Open the Messenger window, or show + focus it if it already exists. The window
/// is created lazily on first use and, thanks to the close handler in `lib.rs`,
/// is hidden (not destroyed) on close so reopening is instant.
#[tauri::command]
pub fn messenger_open(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        crate::messenger::bubble::on_opened(&app);
        return Ok(());
    }
    let url: Url = MESSENGER_URL
        .parse()
        .map_err(|_| "invalid messenger url".to_string())?;
    let handle = app.clone();
    let win = WebviewWindowBuilder::new(&app, MESSENGER_LABEL, WebviewUrl::External(url))
        .title("Messenger")
        .inner_size(1000.0, 760.0)
        .decorations(false)
        .transparent(true)
        .user_agent(DESKTOP_UA)
        .initialization_script(INTERCEPTOR_JS)
        .initialization_script(TITLEBAR_JS)
        .initialization_script(BUBBLE_JS)
        .on_navigation(move |url| on_navigation(&handle, url))
        .build()
        .map_err(|e| e.to_string())?;
    round_corners(&win, 12.0);

    // Route native context-menu selections (from the bubble's right-click menu)
    // back into the bubble module.
    let menu_handle = app.clone();
    win.on_menu_event(move |_win, event| {
        crate::messenger::bubble::on_menu_event(&menu_handle, event.id().as_ref());
    });
    crate::messenger::bubble::on_opened(&app);
    Ok(())
}

/// Destroy the Messenger window to reclaim its RAM (as opposed to the default
/// close, which only hides it). Also closes the preview window if open.
#[tauri::command]
pub fn messenger_close(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(PEEK_LABEL) {
        let _ = win.destroy();
    }
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}
