//! Geometry math, monitor bounds, clamping and the window tween animation.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use super::{
    BubbleState, Rect, ANIM_MS, BUBBLE, FRAME_MS, FULL_H, FULL_W, MARGIN, MIN_FULL, TOP_OFFSET,
};

fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Current window geometry in logical coordinates.
pub(super) fn current_rect(win: &WebviewWindow) -> Rect {
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
pub(super) fn monitor_bounds(win: &WebviewWindow) -> (f64, f64, f64, f64) {
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

pub(super) fn clamp_bubble(win: &WebviewWindow, x: f64, y: f64) -> (f64, f64) {
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
pub(super) fn clamp_full(win: &WebviewWindow, r: Rect) -> Rect {
    let (mx, my, mw, mh) = monitor_bounds(win);
    let w = r.w.min(mw - 2.0 * MARGIN).max(MIN_FULL);
    let h = r.h.min(mh - TOP_OFFSET - MARGIN).max(MIN_FULL);
    let x =
        r.x.clamp(mx + MARGIN, (mx + mw - w - MARGIN).max(mx + MARGIN));
    let y =
        r.y.clamp(my + TOP_OFFSET, (my + mh - h - MARGIN).max(my + TOP_OFFSET));
    Rect { x, y, w, h }
}

/// Open a new animation generation: mark the window as animating (so our own
/// Moved/Resized events aren't read as user drags) and return the generation id.
/// A newer `begin_anim` supersedes this one.
pub(super) fn begin_anim(app: &AppHandle) -> u64 {
    let st = app.state::<BubbleState>();
    st.animating.store(true, Ordering::SeqCst);
    st.anim.fetch_add(1, Ordering::SeqCst) + 1
}

/// Clear the animating flag iff `generation` is still the latest — so a newer
/// animation that superseded us keeps its own guard intact.
pub(super) fn end_anim(app: &AppHandle, generation: u64) {
    let st = app.state::<BubbleState>();
    if st.anim.load(Ordering::SeqCst) == generation {
        st.animating.store(false, Ordering::SeqCst);
    }
}

/// Tween the window from `from` to `to` on a background thread. A newer call
/// bumps `anim`, causing this thread to bail on its next step.
pub(super) fn animate(app: AppHandle, win: WebviewWindow, from: Rect, to: Rect) {
    let generation = begin_anim(&app);
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
        end_anim(&app, generation);
    });
}

/// The rect the full window should tween to: the remembered geometry (clamped to
/// the current monitor) if we have one, else the legacy behavior of a default
/// FULL_W x FULL_H window anchored at the bubble's spot.
pub(super) fn full_target(app: &AppHandle, win: &WebviewWindow, from: Rect) -> Rect {
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
