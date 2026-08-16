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
//!
//! The implementation is split across sibling modules: `link` (routing rules),
//! `geometry` (rect math + the tween), `lifecycle` (state transitions, menu,
//! window events), `settings` (UI-facing getters/setters), and `persist`
//! (disk state). This module owns the shared `BubbleState`, constants, and the
//! public re-exports.

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use std::time::Instant;

use tauri::menu::Menu;
use tauri::Wry;

mod geometry;
mod lifecycle;
mod link;
mod persist;
mod settings;

pub use lifecycle::{
    enter_bubble, expand_click, hide, mark_full, on_menu_event, on_opened, on_shortcut,
    on_window_event, quit, show_menu,
};
pub use link::{LinkAction, LinkRules};
pub use persist::apply_saved_full_geometry;
pub use settings::{
    get_idle_secs, get_link_rules, get_muted, get_shortcut, init_shortcut, set_idle_secs,
    set_link_rules, set_muted, set_shortcut,
};

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
    /// Link-routing rules for clicks inside the Messenger webview. Persisted.
    link_rules: Mutex<LinkRules>,
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
            link_rules: Mutex::new(LinkRules::default()),
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
pub(crate) struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
