//! Tauri command wrappers for the Messenger feature. Each is gated by
//! `require_main` (the untrusted Messenger page must not reach these) and
//! delegates to `bubble`/`window`/`peek`. Window building lives in `window.rs`,
//! the link-preview panel in `peek.rs`, and `on_navigation` in `navigate.rs`.

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::messenger::MESSENGER_LABEL;

// Trusted Rust entry points shared with `bubble` (the global toggle rebuilds the
// window; collapsing/quitting closes the preview). Re-exported here so their
// original `messenger::commands::` paths keep resolving.
pub use super::peek::close_peek;
pub use super::window::open_or_show;

/// Open the Messenger window, or show + focus it if it already exists. The window
/// is created lazily on first use and, thanks to the close handler in `lib.rs`,
/// is hidden (not destroyed) on close so reopening is instant.
#[tauri::command]
pub fn messenger_open(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    crate::security::require_main(&window)?;
    open_or_show(&app)
}

/// Read the current global toggle shortcut (a Tauri accelerator string) so the
/// Messenger settings page can show and record it.
#[tauri::command]
pub fn messenger_get_shortcut(window: WebviewWindow, app: AppHandle) -> Result<String, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_shortcut(&app))
}

/// Rebind the global toggle shortcut. Registers the new accelerator (replacing the
/// old) and persists it. Errors on an unparseable/occupied accelerator.
#[tauri::command]
pub fn messenger_set_shortcut(
    window: WebviewWindow,
    app: AppHandle,
    accelerator: String,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_shortcut(&app, &accelerator)
}

/// Read the current auto-collapse idle timeout (seconds; 0 = disabled) so the
/// Messenger settings page can show it.
#[tauri::command]
pub fn messenger_get_idle_secs(window: WebviewWindow, app: AppHandle) -> Result<u64, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_idle_secs(&app))
}

/// Set the auto-collapse idle timeout (seconds; 0 = disabled) and persist it.
#[tauri::command]
pub fn messenger_set_idle_secs(
    window: WebviewWindow,
    app: AppHandle,
    secs: u64,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_idle_secs(&app, secs);
    Ok(())
}

/// Read whether the unread badge is muted so the settings page can show it.
#[tauri::command]
pub fn messenger_get_muted(window: WebviewWindow, app: AppHandle) -> Result<bool, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_muted(&app))
}

/// Mute or unmute the unread badge and persist it.
#[tauri::command]
pub fn messenger_set_muted(
    window: WebviewWindow,
    app: AppHandle,
    muted: bool,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_muted(&app, muted);
    Ok(())
}

/// Read the link-routing rules so the settings page can show and edit them.
#[tauri::command]
pub fn messenger_get_link_rules(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<crate::messenger::bubble::LinkRules, String> {
    crate::security::require_main(&window)?;
    Ok(crate::messenger::bubble::get_link_rules(&app))
}

/// Replace the link-routing rules from the settings page and persist them.
#[tauri::command]
pub fn messenger_set_link_rules(
    window: WebviewWindow,
    app: AppHandle,
    rules: crate::messenger::bubble::LinkRules,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    crate::messenger::bubble::set_link_rules(&app, rules);
    Ok(())
}

/// Destroy the Messenger window to reclaim its RAM (as opposed to the default
/// close, which only hides it). The preview child webview is destroyed with its
/// parent window, but close it first so the frame state is cleared too.
#[tauri::command]
pub fn messenger_close(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    crate::security::require_main(&window)?;
    close_peek(&app);
    if let Some(win) = app.get_webview_window(MESSENGER_LABEL) {
        win.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}
