//! Link-preview panel: a native child webview overlaid on the Messenger window
//! as an in-window modal. `peek.js` draws the frame; this manages the child.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Url, WebviewBuilder, WebviewUrl};

use super::window::DESKTOP_UA;
use crate::messenger::{MESSENGER_LABEL, PEEK_LABEL, ROUTE_SCHEME};

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

/// Open a URL in the link-preview panel: a native child webview overlaid on the
/// Messenger window (an in-window modal, not a separate OS window). A native
/// webview loads sites that refuse iframing (Facebook, banks, ...), which an
/// in-page iframe modal can't. Reuses the open preview if there is one (navigate
/// and resize), else attaches a new child. `peek.js` draws the modal frame
/// backdrop and header around it in the Messenger page.
pub(super) fn open_peek(app: &AppHandle, url: &str) {
    let target: Url = match url.parse() {
        Ok(u) => u,
        Err(_) => return,
    };
    let Some(mwin) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    // Seed geometry only: peek.js reports the exact CSS-px rect once shown and
    // resizes the child to it, so the child never renders at this size.
    let pos = LogicalPosition::new(PEEK_MARGIN, PEEK_MARGIN + PEEK_HEADER);
    let size = LogicalSize::new(400.0, 400.0);
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
                super::window::mask_webview(pw.inner() as *mut objc2::runtime::AnyObject, 10.0, 0);
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
