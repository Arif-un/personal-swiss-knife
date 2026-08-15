//! Floating "chat-head" bubble for the Messenger window.
//!
//! The Messenger window is a single, decorationless, transparent webview that
//! morphs between two states:
//!
//! * **Full** — a normal window showing facebook.com/messages (default
//!   1000x760; the user's resized size + position are remembered and restored
//!   on the next expand and across app restarts).
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
use tauri_plugin_global_shortcut::GlobalShortcutExt;

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
/// Debounce for persisting the full window's geometry: a drag-resize/move fires
/// a burst of events, so we coalesce them into one disk write after the burst.
const FULL_PERSIST_DEBOUNCE_MS: u64 = 400;
/// Floor below which a captured full-mode geometry is ignored, so a stray event
/// (e.g. a bubble-sized frame) never gets persisted as the restore rect.
const MIN_FULL: f64 = 160.0;
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
const MENU_AUTO: &str = "sk_bubble_auto";

/// Default idle timeout before an unfocused full window auto-collapses to the
/// bubble. Persisted (and overridable) via `bubble.json`; a value of 0 disables
/// auto-collapse entirely.
const DEFAULT_IDLE_SECS: u64 = 60;

/// Default global shortcut that toggles the Messenger window between full and
/// bubble. `CmdOrCtrl` is Cmd on macOS and Ctrl elsewhere. User-rebindable from
/// the Messenger page and persisted (as a Tauri accelerator string) in
/// `bubble.json`. Plain Esc still collapses too (handled in `js/bubble.js`).
const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Escape";

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
    /// Remembered full-window geometry (logical), persisted to disk. Restored on
    /// expand and on a cold app launch so the user's chosen size/spot survives.
    full: Mutex<Option<Rect>>,
    /// Debounce generation for coalescing full-geometry disk writes during a drag.
    full_gen: AtomicU64,
    muted: AtomicBool,
    /// Whether an unfocused full window auto-collapses to the bubble after the
    /// idle timeout. Toggleable from the bubble's right-click menu; persisted.
    auto_collapse: AtomicBool,
    /// Idle timeout (seconds) before auto-collapse. 0 disables it. Persisted.
    auto_collapse_secs: AtomicU64,
    /// Idle-timer generation: bumping it (on refocus or a config change) cancels
    /// any in-flight auto-collapse countdown.
    idle: AtomicU64,
    /// The global toggle accelerator (Tauri accelerator string). `None` until
    /// loaded from disk; falls back to `DEFAULT_SHORTCUT`. Persisted.
    shortcut: Mutex<Option<String>>,
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
            full: Mutex::new(None),
            full_gen: AtomicU64::new(0),
            muted: AtomicBool::new(false),
            auto_collapse: AtomicBool::new(true),
            auto_collapse_secs: AtomicU64::new(DEFAULT_IDLE_SECS),
            idle: AtomicU64::new(0),
            shortcut: Mutex::new(None),
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

/// Clamp a remembered full-window rect so it fits on the current monitor: size is
/// capped to the work area, then the top-left is pulled inside the edges. Guards
/// against a saved geometry from a larger/other display leaving the window
/// off-screen or oversized.
fn clamp_full(win: &WebviewWindow, r: Rect) -> Rect {
    let (mx, my, mw, mh) = monitor_bounds(win);
    let w = r.w.min(mw - 2.0 * MARGIN).max(MIN_FULL);
    let h = r.h.min(mh - TOP_OFFSET - MARGIN).max(MIN_FULL);
    let x =
        r.x.clamp(mx + MARGIN, (mx + mw - w - MARGIN).max(mx + MARGIN));
    let y =
        r.y.clamp(my + TOP_OFFSET, (my + mh - h - MARGIN).max(my + TOP_OFFSET));
    Rect { x, y, w, h }
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
    ensure_loaded(app);
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

/// Sync the tracked mode to Full without any window animation. Called when a fresh
/// full-sized window is built (see `open_or_show`), so `mode` matches the real window
/// after a quit-while-collapsed + rebuild instead of staying stale at Bubble.
pub fn mark_full(app: &AppHandle) {
    *app.state::<BubbleState>().mode.lock().unwrap() = Mode::Full;
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

    let target = full_target(app, &win, from);
    // Persist immediately so the restored geometry survives even if the user
    // expands and quits without ever nudging the window.
    remember_full(app, target);
    animate(app.clone(), win.clone(), from, target);
    let _ = win.set_focus();
}

/// The rect the full window should tween to: the remembered geometry (clamped to
/// the current monitor) if we have one, else the legacy behavior of a default
/// FULL_W x FULL_H window anchored at the bubble's spot.
fn full_target(app: &AppHandle, win: &WebviewWindow, from: Rect) -> Rect {
    if let Some(r) = *app.state::<BubbleState>().full.lock().unwrap() {
        return clamp_full(win, r);
    }
    let (mx, my, mw, mh) = monitor_bounds(win);
    let x = if mw > FULL_W + 2.0 * MARGIN {
        from.x.clamp(mx + MARGIN, mx + mw - FULL_W - MARGIN)
    } else {
        mx + MARGIN
    };
    let y = if mh > FULL_H + TOP_OFFSET + MARGIN {
        from.y.clamp(my + TOP_OFFSET, my + mh - FULL_H - MARGIN)
    } else {
        my + TOP_OFFSET
    };
    Rect {
        x,
        y,
        w: FULL_W,
        h: FULL_H,
    }
}

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

/// Global-shortcut handler: toggle the Messenger window between full and bubble.
/// Window ops (eval / always-on-top / shadow) are macOS-main-thread only, so hop
/// to the main thread before touching the window.
pub fn on_shortcut(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || toggle(&app));
}

fn toggle(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        // Fully freed (messenger_close): rebuild it, which opens full.
        let _ = crate::messenger::commands::open_or_show(app);
        return;
    };
    // Warm-but-hidden or minimized: bring it back as a full window rather than
    // popping a bubble the user can't see.
    if !matches!(win.is_visible(), Ok(true)) || matches!(win.is_minimized(), Ok(true)) {
        let _ = win.unminimize();
        let _ = win.show();
        enter_full(app);
        return;
    }
    let mode = *app.state::<BubbleState>().mode.lock().unwrap();
    match mode {
        Mode::Full => enter_bubble(app),
        Mode::Bubble => enter_full(app),
    }
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

fn toggle_auto_collapse(app: &AppHandle) {
    let st = app.state::<BubbleState>();
    let now = !st.auto_collapse.load(Ordering::SeqCst);
    st.auto_collapse.store(now, Ordering::SeqCst);
    // Cancel any pending countdown when disabling; it re-arms on the next blur
    // once re-enabled.
    st.idle.fetch_add(1, Ordering::SeqCst);
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
    let auto = app
        .state::<BubbleState>()
        .auto_collapse
        .load(Ordering::SeqCst);
    let Ok(auto_it) =
        CheckMenuItem::with_id(app, MENU_AUTO, "Auto-collapse", true, auto, None::<&str>)
    else {
        return;
    };
    let Ok(menu) = Menu::with_items(app, &[&expand, &hide_it, &quit_it, &mute_it, &auto_it]) else {
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
        MENU_AUTO => toggle_auto_collapse(app),
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
        // Track resizes of the full window so the user's chosen size persists.
        // Ignored during our own tween (set_size) and in bubble mode (fixed size).
        WindowEvent::Resized(_) => {
            let app = window.app_handle().clone();
            let st = app.state::<BubbleState>();
            if st.animating.load(Ordering::SeqCst) {
                return;
            }
            if *st.mode.lock().unwrap() != Mode::Full {
                return;
            }
            if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
                remember_full_geometry(&app, &win);
            }
        }
        WindowEvent::Moved(_) => {
            let app = window.app_handle().clone();
            let st = app.state::<BubbleState>();
            // Ignore position changes we caused ourselves (animation).
            if st.animating.load(Ordering::SeqCst) {
                return;
            }
            // In full mode a move just updates the remembered full geometry; the
            // edge-snap logic below is bubble-only.
            if *st.mode.lock().unwrap() != Mode::Bubble {
                if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
                    remember_full_geometry(&app, &win);
                }
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
        // Auto-collapse: while the full window is unfocused for the idle timeout
        // it collapses into the bubble. Regaining focus cancels the countdown.
        WindowEvent::Focused(focused) => {
            let app = window.app_handle().clone();
            if *focused {
                // Cancel any pending countdown; a fresh one arms on the next blur.
                app.state::<BubbleState>()
                    .idle
                    .fetch_add(1, Ordering::SeqCst);
            } else {
                schedule_idle_collapse(&app);
            }
        }
        _ => {}
    }
}

/// Arm the auto-collapse countdown after the full window loses focus. No-op
/// unless we're in full mode with auto-collapse enabled and a non-zero timeout.
/// A newer arm (or a refocus) bumps `idle`, so this thread bails on wake.
fn schedule_idle_collapse(app: &AppHandle) {
    let secs = {
        let st = app.state::<BubbleState>();
        if *st.mode.lock().unwrap() != Mode::Full {
            return;
        }
        if !st.auto_collapse.load(Ordering::SeqCst) {
            return;
        }
        st.auto_collapse_secs.load(Ordering::SeqCst)
    };
    if secs == 0 {
        return;
    }
    let generation = app
        .state::<BubbleState>()
        .idle
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let app = app.clone();
    // A cheap async timer, not an OS thread: rapid focus toggling can arm many
    // countdowns, and detached threads (each with a reserved stack) sleeping the
    // full timeout would accumulate. Stale arms are cancelled by the generation
    // check below.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        {
            let st = app.state::<BubbleState>();
            if st.idle.load(Ordering::SeqCst) != generation {
                return; // refocused or superseded by a newer arm
            }
            if *st.mode.lock().unwrap() != Mode::Full {
                return; // already collapsed some other way
            }
        }
        // Window ops (eval / always-on-top / shadow) must run on the main thread
        // on macOS, so hop back before collapsing. Re-check state there too.
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            {
                let st = app2.state::<BubbleState>();
                if st.idle.load(Ordering::SeqCst) != generation {
                    return;
                }
                if *st.mode.lock().unwrap() != Mode::Full {
                    return;
                }
            }
            let Some(win) = app2.get_webview_window(MESSENGER_LABEL) else {
                return;
            };
            // Only a visible, still-unfocused window collapses: don't pop a
            // bubble the user didn't ask for out of a hidden/minimized window.
            if !matches!(win.is_visible(), Ok(true)) {
                return;
            }
            if matches!(win.is_minimized(), Ok(true)) {
                return;
            }
            if matches!(win.is_focused(), Ok(true)) {
                return;
            }
            enter_bubble(&app2);
        });
    });
}

/// Arm the idle countdown when the full window is first opened, so a launch the
/// user never interacts with still collapses. Refocusing (or the window being
/// focused on open) cancels it via the `Focused` event / the fire-time check.
pub fn on_opened(app: &AppHandle) {
    ensure_loaded(app);
    schedule_idle_collapse(app);
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

/// Update the remembered full geometry in memory only (no disk write).
fn set_full(app: &AppHandle, r: Rect) {
    *app.state::<BubbleState>().full.lock().unwrap() = Some(r);
}

/// Update the remembered full geometry and flush it to disk immediately. Used for
/// discrete events (expand); live drags use `remember_full_geometry`, which
/// debounces the write.
fn remember_full(app: &AppHandle, r: Rect) {
    set_full(app, r);
    persist(app);
}

/// Capture the full window's current geometry as the restore rect, debouncing the
/// disk write across a drag burst. Skips maximized/minimized/tiny frames so the
/// persisted value stays a sane "restore" size.
fn remember_full_geometry(app: &AppHandle, win: &WebviewWindow) {
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
fn ensure_loaded(app: &AppHandle) {
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
    let full = *st.full.lock().unwrap();
    let muted = st.muted.load(Ordering::SeqCst);
    let auto = st.auto_collapse.load(Ordering::SeqCst);
    let secs = st.auto_collapse_secs.load(Ordering::SeqCst);
    let shortcut = st.shortcut.lock().unwrap().clone();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut obj = serde_json::json!({
        "muted": muted,
        "auto_collapse": auto,
        "auto_collapse_secs": secs,
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
