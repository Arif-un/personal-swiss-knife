//! Tauri commands backing the `/utils` page (Cisco Umbrella toggle).

use std::path::Path;
use std::process::Command;

use super::{CiscoStatus, DAEMON_LABEL, DAEMON_PLIST, ORGINFO, ORGINFO_OFF};

/// Current Umbrella state (installed / running / profile-present). Read-only, so
/// no admin auth is needed.
#[tauri::command]
pub fn cisco_status(window: tauri::WebviewWindow) -> Result<CiscoStatus, String> {
    crate::security::require_main(&window)?;
    Ok(status())
}

/// The actual state probe, callable internally (post-toggle) without re-checking
/// the caller window.
fn status() -> CiscoStatus {
    CiscoStatus {
        installed: Path::new(DAEMON_PLIST).exists(),
        running: acumbrella_running(),
        profile_present: Path::new(ORGINFO).exists(),
    }
}

/// Enable or disable Cisco Umbrella: move the Umbrella profile in/out of place
/// and bounce the Secure Client daemon, via one privileged shell command (native
/// macOS admin prompt). The VPN side is unaffected. Returns the refreshed status.
#[tauri::command]
pub async fn cisco_set_enabled(
    window: tauri::WebviewWindow,
    enabled: bool,
) -> Result<CiscoStatus, String> {
    crate::security::require_main(&window)?;
    if !Path::new(DAEMON_PLIST).exists() {
        return Err("Cisco Secure Client is not installed on this machine.".into());
    }
    // The osascript call blocks until the user answers the auth prompt, so keep it
    // off the async runtime worker.
    tauri::async_runtime::spawn_blocking(move || run_toggle(enabled))
        .await
        .map_err(|e| e.to_string())??;
    Ok(status())
}

fn acumbrella_running() -> bool {
    // Absolute path: a GUI-launched app has a minimal PATH, so don't rely on it
    // resolving `pgrep`.
    Command::new("/usr/bin/pgrep")
        .args(["-x", "acumbrellaagent"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_toggle(enabled: bool) -> Result<(), String> {
    // Move the profile in the requested direction, then restart the daemon so the
    // change takes effect. `enable` clears a possible disabled override,
    // `bootstrap` loads the daemon if it isn't (a no-op error when already loaded,
    // suppressed), and `kickstart -k` atomically kills+restarts it - avoiding the
    // async race an explicit `bootout; sleep; bootstrap` has. The `[ -f ] && mv`
    // guard plus trailing `true` keep idempotent no-ops from failing the script;
    // a cancelled auth prompt is surfaced by osascript itself (AppleScript -128).
    let move_step = if enabled {
        format!("[ -f '{ORGINFO_OFF}' ] && mv -f '{ORGINFO_OFF}' '{ORGINFO}'")
    } else {
        format!("[ -f '{ORGINFO}' ] && mv -f '{ORGINFO}' '{ORGINFO_OFF}'")
    };
    let shell = format!(
        "{move_step}; launchctl enable system/{DAEMON_LABEL} 2>/dev/null; \
         launchctl bootstrap system '{DAEMON_PLIST}' 2>/dev/null; \
         launchctl kickstart -k system/{DAEMON_LABEL} 2>/dev/null; true"
    );
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        escape_applescript(&shell)
    );

    let out = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("failed to launch osascript: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(if err.contains("-128") {
            "Authorization cancelled.".into()
        } else if err.is_empty() {
            "Failed to change Cisco Umbrella state.".into()
        } else {
            err.to_string()
        });
    }
    Ok(())
}

/// Escape a string for embedding inside an AppleScript double-quoted literal.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
