//! IPC hardening for app-defined Tauri commands.
//!
//! Tauri v2's capability/permission ACL gates *core* and *plugin* commands, but
//! app-defined `#[tauri::command]`s bypass it entirely: once a window can reach
//! the IPC bridge it may invoke every registered app command, regardless of that
//! window's capability. The Messenger window loads remote, untrusted content
//! (facebook.com) and is granted IPC access so its custom, decorationless title
//! bar can be dragged (`core:window:allow-start-dragging` in
//! `capabilities/messenger.json`). That same bridge would otherwise let a hostile
//! script on the page invoke privileged commands — the root-level Cisco toggle,
//! SSH/keychain access, GitHub, local file writes. Every app command therefore
//! asserts it was called from the trusted main window.

use tauri::WebviewWindow;

/// Label of the only window allowed to invoke app-defined commands. All app UI
/// lives here; the Messenger and preview windows never call app commands (their
/// controls route through the `swissknife-link://` navigation scheme instead).
pub const MAIN_WINDOW: &str = "main";

/// Reject any invocation that did not originate from the trusted main window.
/// Add `window: tauri::WebviewWindow` to a command and call this first.
pub fn require_main(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == MAIN_WINDOW {
        Ok(())
    } else {
        Err("unauthorized".into())
    }
}
