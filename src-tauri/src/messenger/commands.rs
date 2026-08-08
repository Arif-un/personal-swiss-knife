use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
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
const INTERCEPTOR_JS: &str = r#"
(function () {
  var INTERNAL = /(^|\.)facebook\.com$|(^|\.)fbcdn\.net$|(^|\.)messenger\.com$|(^|\.)fb\.com$/;
  function unwrap(raw) {
    try {
      var u = new URL(raw, location.href);
      if (u.hostname === "l.facebook.com" || u.hostname === "lm.facebook.com") {
        var t = u.searchParams.get("u");
        if (t) return t;
      }
      return u.href;
    } catch (e) {
      return raw;
    }
  }
  function isInternal(raw) {
    try { return INTERNAL.test(new URL(raw, location.href).hostname); }
    catch (e) { return true; }
  }
  // Returns true if the link was captured (caller should cancel the default).
  function route(raw, toBrowser) {
    var target = unwrap(raw);
    if (isInternal(target)) return false;
    var mode = toBrowser ? "browser" : "peek";
    location.href = "swissknife-link://route?mode=" + mode + "&url=" + encodeURIComponent(target);
    return true;
  }
  document.addEventListener("click", function (e) {
    if (e.defaultPrevented || e.button !== 0) return;
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (!a) return;
    if (route(a.href, e.shiftKey)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);
})();
"#;

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

/// Open the Messenger window, or show + focus it if it already exists. The window
/// is created lazily on first use and, thanks to the close handler in `lib.rs`,
/// is hidden (not destroyed) on close so reopening is instant.
#[tauri::command]
pub fn messenger_open(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let url: Url = MESSENGER_URL
        .parse()
        .map_err(|_| "invalid messenger url".to_string())?;
    let handle = app.clone();
    WebviewWindowBuilder::new(&app, MESSENGER_LABEL, WebviewUrl::External(url))
        .title("Messenger")
        .inner_size(1000.0, 760.0)
        .user_agent(DESKTOP_UA)
        .initialization_script(INTERCEPTOR_JS)
        .on_navigation(move |url| on_navigation(&handle, url))
        .build()
        .map_err(|e| e.to_string())?;
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
