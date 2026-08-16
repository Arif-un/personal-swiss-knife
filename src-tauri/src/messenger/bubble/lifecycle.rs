//! Window state transitions (full <-> bubble), the native context menu, and the
//! window-event hook driving edge-snap and auto-collapse.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow, Window, WindowEvent};

use super::geometry::{
    animate, begin_anim, clamp_bubble, current_rect, end_anim, full_target, monitor_bounds,
};
use super::persist::{
    ensure_loaded, persist, remember_full, remember_full_geometry, remember_pos, set_window_shadow,
};
use super::{
    BubbleState, Mode, Rect, BUBBLE, CLICK_SUPPRESS_MS, MARGIN, MENU_AUTO, MENU_EXPAND, MENU_HIDE,
    MENU_MUTE, MENU_QUIT, SNAP_DEBOUNCE_MS, TOP_OFFSET,
};
use crate::messenger::MESSENGER_LABEL;

/// Collapse the full window into the floating bubble. `smooth` runs the shrink
/// tween (manual collapse, where the window is focused and in front); when it is
/// false the window snaps straight to the bubble. Auto-collapse fires while
/// another app is focused, so a tween there would raise the full chat UI to the
/// front for its duration and flash over whatever you're doing.
pub fn enter_bubble(app: &AppHandle, smooth: bool) {
    let Some(win) = app.get_webview_window(MESSENGER_LABEL) else {
        return;
    };
    ensure_loaded(app);
    // A link preview at bubble size would be nonsense; drop it on collapse.
    crate::messenger::commands::close_peek(app);
    *app.state::<BubbleState>().mode.lock().unwrap() = Mode::Bubble;

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
    let to = Rect {
        x: tx,
        y: ty,
        w: BUBBLE,
        h: BUBBLE,
    };

    if smooth {
        let _ = win.set_always_on_top(true);
        let _ = win.set_visible_on_all_workspaces(true);
        // A transparent window's native shadow is a rectangle; drop it so no
        // square halo shows behind the round bubble (the circle draws its own).
        set_window_shadow(&win, false);
        animate(app.clone(), win, from, to);
        return;
    }

    // Instant collapse: resize to the bubble BEFORE raising to front, so an
    // unfocused full window never pops its chat UI over the app you're using. The
    // `animating` guard is held across the resize (as `animate` does) so the
    // Moved/Resized events it emits aren't mistaken for a user drag, which would
    // schedule a stray edge-snap.
    let generation = begin_anim(app);
    let _ = win.set_size(LogicalSize::new(to.w, to.h));
    let _ = win.set_position(LogicalPosition::new(to.x, to.y));
    let _ = win.set_always_on_top(true);
    let _ = win.set_visible_on_all_workspaces(true);
    set_window_shadow(&win, false);
    let app = app.clone();
    std::thread::spawn(move || {
        // Let the async window events settle before dropping the guard.
        std::thread::sleep(Duration::from_millis(SNAP_DEBOUNCE_MS + 40));
        end_anim(&app, generation);
    });
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
        Mode::Full => enter_bubble(app, true),
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
    // Peek is a child webview of the Messenger window (torn down with its parent),
    // but close it explicitly so its modal frame state is cleared too.
    crate::messenger::commands::close_peek(app);
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
            // An open link preview resizes itself: peek.js re-reports its rect on
            // the page's own `resize` event (see commands.rs::open_peek).
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
            enter_bubble(&app2, false);
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
