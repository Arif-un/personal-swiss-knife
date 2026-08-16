//! Settings getters/setters exposed to the Messenger settings page: toggle
//! shortcut, auto-collapse idle timeout, badge mute, and link-routing rules.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use super::persist::{ensure_loaded, persist};
use super::{BubbleState, LinkRules, DEFAULT_SHORTCUT};
use crate::messenger::MESSENGER_LABEL;

/// The current toggle accelerator (persisted value, or the default).
pub fn get_shortcut(app: &AppHandle) -> String {
    ensure_loaded(app);
    app.state::<BubbleState>()
        .shortcut
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string())
}

/// Register `accel` as the global toggle shortcut, unregistering whatever was
/// bound before. The new accelerator becomes the remembered one only on success,
/// so a bad string leaves the old binding intact.
pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let prev = app.state::<BubbleState>().shortcut.lock().unwrap().clone();
    // Already bound to this combo: nothing to do (re-registering errors).
    if prev.as_deref() == Some(accel) {
        return Ok(());
    }
    // Register the new accelerator BEFORE unregistering the old one, so a bad or
    // OS-occupied combo fails with the existing binding still intact.
    gs.register(accel).map_err(|e| e.to_string())?;
    if let Some(prev) = prev {
        let _ = gs.unregister(prev.as_str());
    }
    *app.state::<BubbleState>().shortcut.lock().unwrap() = Some(accel.to_string());
    Ok(())
}

/// The current auto-collapse idle timeout in seconds. 0 means auto-collapse is
/// off (either the timeout is 0 or the menu toggle disabled it), so the UI shows
/// a single number where 0 == disabled.
pub fn get_idle_secs(app: &AppHandle) -> u64 {
    ensure_loaded(app);
    let st = app.state::<BubbleState>();
    if !st.auto_collapse.load(Ordering::SeqCst) {
        return 0;
    }
    st.auto_collapse_secs.load(Ordering::SeqCst)
}

/// Set the auto-collapse idle timeout (seconds) from the UI. 0 disables
/// auto-collapse; any positive value enables it, keeping the bubble menu's
/// checkbox in sync. Cancels any in-flight countdown so the next blur re-arms
/// with the new timeout.
pub fn set_idle_secs(app: &AppHandle, secs: u64) {
    let st = app.state::<BubbleState>();
    if secs == 0 {
        st.auto_collapse.store(false, Ordering::SeqCst);
    } else {
        st.auto_collapse.store(true, Ordering::SeqCst);
        st.auto_collapse_secs.store(secs, Ordering::SeqCst);
    }
    st.idle.fetch_add(1, Ordering::SeqCst);
    persist(app);
}

/// Whether the unread badge is muted (hidden), for the settings page.
pub fn get_muted(app: &AppHandle) -> bool {
    ensure_loaded(app);
    app.state::<BubbleState>().muted.load(Ordering::SeqCst)
}

/// Set the badge mute state from the UI, mirroring the bubble menu's checkbox:
/// update the live overlay and persist.
pub fn set_muted(app: &AppHandle, muted: bool) {
    let st = app.state::<BubbleState>();
    st.muted.store(muted, Ordering::SeqCst);
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        let _ = win.eval(format!("window.__skSetMuted&&window.__skSetMuted({muted})"));
    }
    persist(app);
}

/// Rebind the toggle shortcut from the UI: register, then persist.
pub fn set_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    register_shortcut(app, accel)?;
    persist(app);
    Ok(())
}

/// The current link-routing rules, for the settings page and `on_navigation`.
pub fn get_link_rules(app: &AppHandle) -> LinkRules {
    ensure_loaded(app);
    app.state::<BubbleState>()
        .link_rules
        .lock()
        .unwrap()
        .clone()
}

/// Replace the link-routing rules from the settings page and persist them.
pub fn set_link_rules(app: &AppHandle, rules: LinkRules) {
    *app.state::<BubbleState>().link_rules.lock().unwrap() = rules;
    persist(app);
}

/// Register the persisted (or default) shortcut at startup.
pub fn init_shortcut(app: &AppHandle) {
    let accel = get_shortcut(app);
    // get_shortcut -> ensure_loaded populates st.shortcut from disk WITHOUT
    // registering with the OS. Clear it first so register_shortcut's dedup guard
    // (which treats the remembered value as proof of OS registration) can't
    // short-circuit and skip the actual gs.register at startup.
    *app.state::<BubbleState>().shortcut.lock().unwrap() = None;
    if let Err(e) = register_shortcut(app, &accel) {
        eprintln!("messenger: failed to register toggle shortcut {accel:?}: {e}");
    }
}
