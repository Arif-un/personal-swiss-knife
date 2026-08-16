//! The Messenger window's `on_navigation` handler: it only ever intercepts the
//! injected overlays' `swissknife-link://` signals (window controls, peek
//! geometry, and link-routing intents) and applies the user's routing rules.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Url};
use tauri_plugin_opener::OpenerExt;

use super::peek::{close_peek, open_peek};
use crate::messenger::{MESSENGER_LABEL, PEEK_LABEL, ROUTE_SCHEME};

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
pub(super) fn on_navigation(app: &AppHandle, url: &Url) -> bool {
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
