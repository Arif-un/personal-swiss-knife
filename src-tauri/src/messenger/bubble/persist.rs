//! Disk persistence of bubble state and the full-window restore geometry.

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use super::geometry::{clamp_full, current_rect};
use super::{BubbleState, LinkRules, Rect, FULL_PERSIST_DEBOUNCE_MS, MIN_FULL};

/// Apply the saved full-window geometry to a freshly built window (cold launch),
/// so a restart restores the user's chosen size + position. No-op if nothing is
/// remembered, leaving the builder's default size.
pub fn apply_saved_full_geometry(app: &AppHandle, win: &WebviewWindow) {
    ensure_loaded(app);
    let Some(r) = *app.state::<BubbleState>().full.lock().unwrap() else {
        return;
    };
    let t = clamp_full(win, r);
    let _ = win.set_size(LogicalSize::new(t.w, t.h));
    let _ = win.set_position(LogicalPosition::new(t.x, t.y));
    set_full(app, t);
}

pub(super) fn remember_pos(app: &AppHandle, x: f64, y: f64) {
    *app.state::<BubbleState>().pos.lock().unwrap() = Some((x, y));
    persist(app);
}

/// Update the remembered full geometry in memory only (no disk write).
pub(super) fn set_full(app: &AppHandle, r: Rect) {
    *app.state::<BubbleState>().full.lock().unwrap() = Some(r);
}

/// Update the remembered full geometry and flush it to disk immediately. Used for
/// discrete events (expand); live drags use `remember_full_geometry`, which
/// debounces the write.
pub(super) fn remember_full(app: &AppHandle, r: Rect) {
    set_full(app, r);
    persist(app);
}

/// Capture the full window's current geometry as the restore rect, debouncing the
/// disk write across a drag burst. Skips maximized/minimized/tiny frames so the
/// persisted value stays a sane "restore" size.
pub(super) fn remember_full_geometry(app: &AppHandle, win: &WebviewWindow) {
    if matches!(win.is_minimized(), Ok(true)) || matches!(win.is_maximized(), Ok(true)) {
        return;
    }
    let r = current_rect(win);
    if r.w < MIN_FULL || r.h < MIN_FULL {
        return;
    }
    set_full(app, r);
    let n = app
        .state::<BubbleState>()
        .full_gen
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(FULL_PERSIST_DEBOUNCE_MS));
        if app.state::<BubbleState>().full_gen.load(Ordering::SeqCst) != n {
            return; // superseded by a later move/resize in the same burst
        }
        persist(&app);
    });
}

fn state_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("bubble.json"))
}

/// Load persisted position + mute + auto-collapse settings once per run.
pub(super) fn ensure_loaded(app: &AppHandle) {
    let st = app.state::<BubbleState>();
    if st.loaded.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(path) = state_file(app) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    if let (Some(x), Some(y)) = (
        v.get("x").and_then(|x| x.as_f64()),
        v.get("y").and_then(|y| y.as_f64()),
    ) {
        *st.pos.lock().unwrap() = Some((x, y));
    }
    if let (Some(x), Some(y), Some(w), Some(h)) = (
        v.get("fx").and_then(|n| n.as_f64()),
        v.get("fy").and_then(|n| n.as_f64()),
        v.get("fw").and_then(|n| n.as_f64()),
        v.get("fh").and_then(|n| n.as_f64()),
    ) {
        *st.full.lock().unwrap() = Some(Rect { x, y, w, h });
    }
    if let Some(m) = v.get("muted").and_then(|m| m.as_bool()) {
        st.muted.store(m, Ordering::SeqCst);
    }
    if let Some(a) = v.get("auto_collapse").and_then(|a| a.as_bool()) {
        st.auto_collapse.store(a, Ordering::SeqCst);
    }
    if let Some(s) = v.get("auto_collapse_secs").and_then(|s| s.as_u64()) {
        st.auto_collapse_secs.store(s, Ordering::SeqCst);
    }
    if let Some(s) = v.get("shortcut").and_then(|s| s.as_str()) {
        *st.shortcut.lock().unwrap() = Some(s.to_string());
    }
    if let Some(lr) = v.get("link_rules").cloned() {
        if let Ok(rules) = serde_json::from_value::<LinkRules>(lr) {
            *st.link_rules.lock().unwrap() = rules;
        }
    }
}

/// Toggle the native window shadow. On macOS a transparent window's shadow is a
/// rectangle, which looks wrong around the round bubble, so we drop it in bubble
/// mode and restore it when expanded. No-op on other platforms (the overlay's
/// CSS shadow covers the bubble regardless).
#[cfg(target_os = "macos")]
pub(super) fn set_window_shadow(win: &WebviewWindow, on: bool) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let Ok(ns_window) = win.ns_window() else {
        return;
    };
    let ns_window = ns_window as *mut AnyObject;
    unsafe {
        let _: () = msg_send![ns_window, setHasShadow: on];
        let _: () = msg_send![ns_window, invalidateShadow];
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn set_window_shadow(_win: &WebviewWindow, _on: bool) {}

pub(super) fn persist(app: &AppHandle) {
    let Some(path) = state_file(app) else {
        return;
    };
    let st = app.state::<BubbleState>();
    let pos = *st.pos.lock().unwrap();
    let full = *st.full.lock().unwrap();
    let muted = st.muted.load(Ordering::SeqCst);
    let auto = st.auto_collapse.load(Ordering::SeqCst);
    let secs = st.auto_collapse_secs.load(Ordering::SeqCst);
    let shortcut = st.shortcut.lock().unwrap().clone();
    let link_rules = serde_json::to_value(&*st.link_rules.lock().unwrap()).unwrap_or_default();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut obj = serde_json::json!({
        "muted": muted,
        "auto_collapse": auto,
        "auto_collapse_secs": secs,
        "link_rules": link_rules,
    });
    if let Some(s) = shortcut {
        obj["shortcut"] = s.into();
    }
    // Only persist x/y once a real bubble position exists. Writing (0,0) for an
    // unset `pos` would reload as a genuine top-left position and defeat the
    // default top-right placement on the first collapse.
    if let Some((x, y)) = pos {
        obj["x"] = x.into();
        obj["y"] = y.into();
    }
    if let Some(r) = full {
        obj["fx"] = r.x.into();
        obj["fy"] = r.y.into();
        obj["fw"] = r.w.into();
        obj["fh"] = r.h.into();
    }
    let _ = std::fs::write(&path, obj.to_string());
}
