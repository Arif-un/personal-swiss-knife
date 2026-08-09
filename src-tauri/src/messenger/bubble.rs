//! Floating "chat-head" bubble for the Messenger window.
//!
//! The Messenger window is a single, decorationless, transparent webview that
//! morphs between two states:
//!
//! * **Full** — a normal 1000x760 window showing facebook.com/messages.
//! * **Bubble** — a tiny (~76px) always-on-top circle drawn by an injected
//!   overlay (see `js/bubble.js`). The FB page stays loaded behind the opaque
//!   circle, so the unread count keeps updating live.
//!
//! There is no second window: `enter_bubble` / `enter_full` tween the native
//! window's size + position (cross-platform, driven from Rust) and push the UI
//! state to the page with `WebviewWindow::eval`. The injected overlay uses
//! `data-tauri-drag-region` for native dragging; on drag release we snap the
//! bubble to the nearest left/right screen edge and remember its position.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow, Window, WindowEvent, Wry,
};

use crate::messenger::{MESSENGER_LABEL, PEEK_LABEL};

/// Bubble diameter (logical px). The circle drawn by the overlay is smaller so
/// its drop shadow has room inside the window bounds.
const BUBBLE: f64 = 56.0;
const FULL_W: f64 = 1000.0;
const FULL_H: f64 = 760.0;
/// Gap kept between the bubble/window and the screen edges.
const MARGIN: f64 = 12.0;
/// Top inset so the bubble clears the macOS menu bar (Monitor exposes no work
/// area in Tauri v2, so we approximate).
const TOP_OFFSET: f64 = 40.0;
const ANIM_MS: u64 = 240;
/// Frame budget between tween updates (~120fps). The tween is driven by real
/// elapsed time, not a fixed step count, so a slow frame never stalls or drifts
/// the animation — it just draws fewer frames while still finishing in ANIM_MS.
const FRAME_MS: u64 = 8;
/// After the last native drag move, wait this long before snapping to an edge.
const SNAP_DEBOUNCE_MS: u64 = 180;
/// A `click` that lands within this long after the last drag move is treated as
/// the tail of a drag, not a real click, so dragging never accidentally expands.
const CLICK_SUPPRESS_MS: u128 = 280;

const MENU_EXPAND: &str = "sk_bubble_expand";
const MENU_HIDE: &str = "sk_bubble_hide";
const MENU_QUIT: &str = "sk_bubble_quit";
const MENU_MUTE: &str = "sk_bubble_mute";

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Full,
    Bubble,
}

/// Per-app bubble state, stored via `app.manage`.
pub struct BubbleState {
    mode: Mutex<Mode>,
    /// Animation generation: bumping it cancels any in-flight tween thread.
    anim: AtomicU64,
    /// True while a tween is running, so native `Moved` events (our own
    /// `set_position` calls) don't get mistaken for user drags.
    animating: AtomicBool,
    /// Move-event counter used to debounce edge-snapping.
    moves: AtomicU64,
    /// Timestamp of the last user drag move (for click-vs-drag suppression).
    last_move: Mutex<Option<Instant>>,
    /// Remembered bubble position (logical, top-left), persisted to disk.
    pos: Mutex<Option<(f64, f64)>>,
    muted: AtomicBool,
    /// Whether the persisted position/mute have been loaded from disk yet.
    loaded: AtomicBool,
    /// Keeps the most recent context menu alive while it is displayed.
    menu: Mutex<Option<Menu<Wry>>>,
}

impl BubbleState {
    pub fn new() -> Self {
        Self {
            mode: Mutex::new(Mode::Full),
            anim: AtomicU64::new(0),
            animating: AtomicBool::new(false),
            moves: AtomicU64::new(0),
            last_move: Mutex::new(None),
            pos: Mutex::new(None),
            muted: AtomicBool::new(false),
            loaded: AtomicBool::new(false),
            menu: Mutex::new(None),
        }
    }
}

impl Default for BubbleState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Current window geometry in logical coordinates.
fn current_rect(win: &WebviewWindow) -> Rect {
    let scale = win.scale_factor().unwrap_or(1.0);
    let (px, py) = win
        .outer_position()
        .map(|p| (p.x as f64, p.y as f64))
        .unwrap_or((0.0, 0.0));
    let (sw, sh) = win
        .inner_size()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((BUBBLE, BUBBLE));
    Rect {
        x: px / scale,
        y: py / scale,
        w: sw / scale,
        h: sh / scale,
    }
}

/// Monitor bounds (logical) for the monitor the window is on. Falls back to the
/// primary monitor, then a sane default.
fn monitor_bounds(win: &WebviewWindow) -> (f64, f64, f64, f64) {
    let monitor = win
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| win.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let s = m.scale_factor();
        let p = m.position();
        let sz = m.size();
        (
            p.x as f64 / s,
            p.y as f64 / s,
            sz.width as f64 / s,
            sz.height as f64 / s,
        )
    } else {
        (0.0, 0.0, 1440.0, 900.0)
    }
}

fn clamp_bubble(win: &WebviewWindow, x: f64, y: f64) -> (f64, f64) {
    let (mx, my, mw, mh) = monitor_bounds(win);
    let cx = x.clamp(mx + MARGIN, (mx + mw - BUBBLE - MARGIN).max(mx + MARGIN));
    let cy = y.clamp(
        my + TOP_OFFSET,
        (my + mh - BUBBLE - MARGIN).max(my + TOP_OFFSET),
    );
    (cx, cy)
}

/// Tween the window from `from` to `to` on a background thread. A newer call
/// bumps `anim`, causing this thread to bail on its next step.
fn animate(app: AppHandle, win: WebviewWindow, from: Rect, to: Rect) {
    let generation = {
        let st = app.state::<BubbleState>();
        st.animating.store(true, Ordering::SeqCst);
        st.anim.fetch_add(1, Ordering::SeqCst) + 1
    };
    std::thread::spawn(move || {
        let start = Instant::now();
        let dur = ANIM_MS as f64;
        loop {
            if app.state::<BubbleState>().anim.load(Ordering::SeqCst) != generation {
                return; // superseded by a newer animation
            }
            // Progress from wall-clock elapsed, so a slow set_size/set_position
            // frame just skips ahead instead of stretching the whole tween.
            let raw = (start.elapsed().as_millis() as f64 / dur).min(1.0);
            let t = ease_out_cubic(raw);
            // Size before position: both are top-left anchored in Tauri's logical
            // coords, and keeping the order fixed avoids a two-phase visible jump.
            let _ = win.set_size(LogicalSize::new(
                lerp(from.w, to.w, t),
                lerp(from.h, to.h, t),
            ));
            let _ = win.set_position(LogicalPosition::new(
                lerp(from.x, to.x, t),
                lerp(from.y, to.y, t),
            ));
            if raw >= 1.0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(FRAME_MS));
        }
        let _ = win.set_size(LogicalSize::new(to.w, to.h));
        let _ = win.set_position(LogicalPosition::new(to.x, to.y));
        let st = app.state::<BubbleState>();
        if st.anim.load(Ordering::SeqCst) == generation {
            st.animating.store(false, Ordering::SeqCst);
        }
    });
}

/// Collapse the full window into the floating bubble.
pub fn enter_bubble(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    ensure_loaded(app, &win);
    *app.state::<BubbleState>().mode.lock().unwrap() = Mode::Bubble;

    let _ = win.set_always_on_top(true);
    let _ = win.set_visible_on_all_workspaces(true);
    // A transparent window's native shadow is a rectangle; drop it so no square
    // halo shows behind the round bubble (the circle draws its own CSS shadow).
    set_window_shadow(&win, false);
    let muted = app.state::<BubbleState>().muted.load(Ordering::SeqCst);
    let _ = win.eval(format!(
        "window.__skSetState&&window.__skSetState('bubble');window.__skSetMuted&&window.__skSetMuted({muted})"
    ));

    let from = current_rect(&win);
    let (tx, ty) = {
        let remembered = *app.state::<BubbleState>().pos.lock().unwrap();
        match remembered {
            Some((x, y)) => clamp_bubble(&win, x, y),
            None => {
                let (mx, my, mw, _mh) = monitor_bounds(&win);
                (mx + mw - BUBBLE - MARGIN, my + TOP_OFFSET)
            }
        }
    };
    remember_pos(app, tx, ty);
    animate(
        app.clone(),
        win,
        from,
        Rect {
            x: tx,
            y: ty,
            w: BUBBLE,
            h: BUBBLE,
        },
    );
}

/// Grow the bubble back to the full Messenger window, anchored at the bubble's
/// spot and clamped to stay on screen.
pub fn enter_full(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    let from = current_rect(&win);
    {
        let st = app.state::<BubbleState>();
        *st.mode.lock().unwrap() = Mode::Full;
        *st.pos.lock().unwrap() = Some((from.x, from.y));
    }
    remember_pos(app, from.x, from.y);

    let _ = win.set_always_on_top(false);
    let _ = win.set_visible_on_all_workspaces(false);
    set_window_shadow(&win, true);
    let _ = win.eval("window.__skSetState&&window.__skSetState('full')");

    let (mx, my, mw, mh) = monitor_bounds(&win);
    let tx = if mw > FULL_W + 2.0 * MARGIN {
        from.x.clamp(mx + MARGIN, mx + mw - FULL_W - MARGIN)
    } else {
        mx + MARGIN
    };
    let ty = if mh > FULL_H + TOP_OFFSET + MARGIN {
        from.y.clamp(my + TOP_OFFSET, my + mh - FULL_H - MARGIN)
    } else {
        my + TOP_OFFSET
    };
    animate(
        app.clone(),
        win.clone(),
        from,
        Rect {
            x: tx,
            y: ty,
            w: FULL_W,
            h: FULL_H,
        },
    );
    let _ = win.set_focus();
}

/// Left-click on the bubble. Ignored if it is the tail end of a drag.
pub fn expand_click(app: &AppHandle) {
    let recent_drag = app
        .state::<BubbleState>()
        .last_move
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis() < CLICK_SUPPRESS_MS)
        .unwrap_or(false);
    if recent_drag {
        return;
    }
    enter_full(app);
}

/// Hide the window but keep it warm (matches the native close handler).
pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        let _ = win.hide();
    }
}

/// Destroy the Messenger (and preview) windows to reclaim RAM.
pub fn quit(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(PEEK_LABEL) {
        let _ = win.destroy();
    }
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        let _ = win.destroy();
    }
}

fn toggle_mute(app: &AppHandle) {
    let st = app.state::<BubbleState>();
    let now = !st.muted.load(Ordering::SeqCst);
    st.muted.store(now, Ordering::SeqCst);
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        let _ = win.eval(format!("window.__skSetMuted&&window.__skSetMuted({now})"));
    }
    persist(app);
}

/// Build and pop up the native right-click menu (kept native because the bubble
/// window is far too small to host an HTML menu).
pub fn show_menu(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    let muted = app.state::<BubbleState>().muted.load(Ordering::SeqCst);
    let Ok(expand) = MenuItem::with_id(app, MENU_EXPAND, "Expand", true, None::<&str>) else {
        return;
    };
    let Ok(hide_it) = MenuItem::with_id(app, MENU_HIDE, "Close (keep warm)", true, None::<&str>)
    else {
        return;
    };
    let Ok(quit_it) = MenuItem::with_id(app, MENU_QUIT, "Quit Messenger", true, None::<&str>)
    else {
        return;
    };
    let Ok(mute_it) =
        CheckMenuItem::with_id(app, MENU_MUTE, "Mute badge", true, muted, None::<&str>)
    else {
        return;
    };
    let Ok(menu) = Menu::with_items(app, &[&expand, &hide_it, &quit_it, &mute_it]) else {
        return;
    };
    let _ = win.popup_menu(&menu);
    // Keep the menu alive while it is shown; a dropped Menu closes immediately.
    *app.state::<BubbleState>().menu.lock().unwrap() = Some(menu);
}

/// Dispatch a context-menu selection. Registered per-window in `messenger_open`.
pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_EXPAND => enter_full(app),
        MENU_HIDE => hide(app),
        MENU_QUIT => quit(app),
        MENU_MUTE => toggle_mute(app),
        _ => {}
    }
}

/// Global window-event hook for the Messenger window (wired from `lib.rs`).
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    match event {
        // Keep the window warm: closing hides instead of destroying.
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        WindowEvent::Moved(_) => {
            let app = window.app_handle().clone();
            let st = app.state::<BubbleState>();
            // Ignore position changes we caused ourselves (animation) or that
            // happen while the window is in full mode.
            if st.animating.load(Ordering::SeqCst) {
                return;
            }
            if *st.mode.lock().unwrap() != Mode::Bubble {
                return;
            }
            *st.last_move.lock().unwrap() = Some(Instant::now());
            let n = st.moves.fetch_add(1, Ordering::SeqCst) + 1;
            let app2 = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(SNAP_DEBOUNCE_MS));
                let st = app2.state::<BubbleState>();
                if st.moves.load(Ordering::SeqCst) != n || st.animating.load(Ordering::SeqCst) {
                    return; // moved again, or an animation started
                }
                if *st.mode.lock().unwrap() != Mode::Bubble {
                    return;
                }
                if let Some(win) = app2.get_webview_window(MESSENGER_LABEL) {
                    snap(&app2, &win);
                }
            });
        }
        _ => {}
    }
}

/// Snap the bubble to the nearest left/right edge, keeping its vertical spot.
fn snap(app: &AppHandle, win: &WebviewWindow) {
    let r = current_rect(win);
    let (mx, my, mw, mh) = monitor_bounds(win);
    let center = r.x + BUBBLE / 2.0;
    let tx = if center < mx + mw / 2.0 {
        mx + MARGIN
    } else {
        mx + mw - BUBBLE - MARGIN
    };
    let ty = r.y.clamp(
        my + TOP_OFFSET,
        (my + mh - BUBBLE - MARGIN).max(my + TOP_OFFSET),
    );
    remember_pos(app, tx, ty);
    animate(
        app.clone(),
        win.clone(),
        r,
        Rect {
            x: tx,
            y: ty,
            w: BUBBLE,
            h: BUBBLE,
        },
    );
}

fn remember_pos(app: &AppHandle, x: f64, y: f64) {
    *app.state::<BubbleState>().pos.lock().unwrap() = Some((x, y));
    persist(app);
}

fn state_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("bubble.json"))
}

/// Load persisted position + mute state once per run.
fn ensure_loaded(app: &AppHandle, _win: &WebviewWindow) {
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
    if let Some(m) = v.get("muted").and_then(|m| m.as_bool()) {
        st.muted.store(m, Ordering::SeqCst);
    }
}

/// Toggle the native window shadow. On macOS a transparent window's shadow is a
/// rectangle, which looks wrong around the round bubble, so we drop it in bubble
/// mode and restore it when expanded. No-op on other platforms (the overlay's
/// CSS shadow covers the bubble regardless).
#[cfg(target_os = "macos")]
fn set_window_shadow(win: &WebviewWindow, on: bool) {
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
fn set_window_shadow(_win: &WebviewWindow, _on: bool) {}

fn persist(app: &AppHandle) {
    let Some(path) = state_file(app) else {
        return;
    };
    let st = app.state::<BubbleState>();
    let pos = *st.pos.lock().unwrap();
    let muted = st.muted.load(Ordering::SeqCst);
    let (x, y) = pos.unwrap_or((0.0, 0.0));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, format!("{{\"x\":{x},\"y\":{y},\"muted\":{muted}}}"));
}
