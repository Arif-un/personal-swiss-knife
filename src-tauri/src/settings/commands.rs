//! Tauri commands for branding + whole-app backup/restore.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use super::Branding;

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn branding_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("branding.json"))
}

#[tauri::command]
pub fn branding_get(window: WebviewWindow, app: AppHandle) -> Result<Branding, String> {
    crate::security::require_main(&window)?;
    // Missing / corrupt branding just falls back to defaults.
    Ok(std::fs::read_to_string(branding_path(&app)?)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn branding_set(
    window: WebviewWindow,
    app: AppHandle,
    branding: Branding,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = branding_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&branding).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ backup/restore

/// One portable backup file: every config JSON plus the SSH keychain secrets.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bundle {
    version: u32,
    /// `file name -> parsed JSON` for each `*.json` in the app data dir.
    files: BTreeMap<String, serde_json::Value>,
    /// `key id (identity file path) -> passphrase` from the OS keychain.
    secrets: BTreeMap<String, String>,
}

/// Filename prefix for the pre-import snapshot of current settings, written
/// before any overwrite so a mistaken restore is recoverable (re-import this
/// file). Each snapshot is timestamped so a second mistaken import cannot clobber
/// the snapshot from the first — otherwise the only copy of the original settings
/// would be lost. Skipped by `build_bundle` (prefix match) so snapshots never nest
/// inside a fresh export/snapshot.
const IMPORT_SNAPSHOT_PREFIX: &str = "pre-import-backup";

/// Write `json` with owner-only perms (0600) on unix — it may contain plaintext
/// SSH passphrases, so it must never be world-readable.
fn write_secret_file(path: &std::path::Path, json: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // `mode` only applies on create, so also tighten an existing file.
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| e.to_string())?;
        f.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Collect the current settings + secrets into one bundle (for export and for the
/// pre-import snapshot).
fn build_bundle(app: &AppHandle) -> Result<Bundle, String> {
    let dir = data_dir(app)?;
    let mut files = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let (Some(name), Ok(txt)) = (
                p.file_name().and_then(|n| n.to_str()),
                std::fs::read_to_string(&p),
            ) else {
                continue;
            };
            if name.starts_with(IMPORT_SNAPSHOT_PREFIX) {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&txt) {
                files.insert(name.to_string(), val);
            }
        }
    }
    Ok(Bundle {
        version: 1,
        files,
        secrets: collect_secrets(app),
    })
}

/// Best-effort pull of stored SSH passphrases, keyed by the identity file path
/// they were stored under (see `ssh::keychain`). Enumerated via the known hosts
/// because the keychain has no listing API.
fn collect_secrets(app: &AppHandle) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut hosts = crate::ssh::config::parse_ssh_config();
    if let Ok(store) = data_dir(app).map(|d| d.join("hosts.json")) {
        if let Ok(app_hosts) = crate::ssh::config::load_app_hosts(&store) {
            hosts.extend(app_hosts);
        }
    }
    for h in hosts {
        if let Some(id) = h.identity_file.as_deref() {
            if !id.is_empty() && !out.contains_key(id) {
                if let Some(secret) = crate::ssh::keychain::get_passphrase(id) {
                    out.insert(id.to_string(), secret);
                }
            }
        }
    }
    out
}

/// Export all settings + secrets to a user-chosen JSON file. Returns the written
/// path, or `None` if the save dialog was cancelled.
#[tauri::command]
pub async fn settings_export(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<Option<String>, String> {
    crate::security::require_main(&window)?;
    // build_bundle reads the keychain per host, so keep it off the async worker.
    let bundle_app = app.clone();
    let json = tauri::async_runtime::spawn_blocking(move || {
        let bundle = build_bundle(&bundle_app)?;
        serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // blocking_save_file must run off the main thread (it drives the native panel).
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("JSON", &["json"])
            .set_file_name("swiss-knife-settings.json")
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = PathBuf::from(fp.to_string());
    // Contains plaintext SSH passphrases; write it owner-only. Off the worker too.
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || write_secret_file(&write_path, &json))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Filter an imported `hosts.json` value to hosts that pass the same validation
/// `host_save` enforces (see `ssh::commands::validate_host`) — dropping any whose
/// alias/user/hostname could inject an ssh/scp option on the next deploy. A value
/// that is not a host array is returned unchanged (nothing to sanitize).
fn sanitized_hosts(val: &serde_json::Value) -> Result<serde_json::Value, String> {
    let Ok(hosts) = serde_json::from_value::<Vec<crate::ssh::Host>>(val.clone()) else {
        return Ok(val.clone());
    };
    let safe: Vec<crate::ssh::Host> = hosts
        .into_iter()
        .filter(|h| crate::ssh::commands::validate_host(h).is_ok())
        .collect();
    serde_json::to_value(safe).map_err(|e| e.to_string())
}

/// Import a backup file: overwrite each config JSON in the app data dir and
/// restore keychain secrets. Returns `false` if the pick dialog was cancelled.
#[tauri::command]
pub async fn settings_import(window: WebviewWindow, app: AppHandle) -> Result<bool, String> {
    crate::security::require_main(&window)?;
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("JSON", &["json"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(fp) = picked else {
        return Ok(false);
    };
    let import_path = PathBuf::from(fp.to_string());
    // The snapshot build_bundle (keychain reads), the config file writes and the
    // set_passphrase loop are all blocking, so run the whole restore off the worker.
    tauri::async_runtime::spawn_blocking(move || -> Result<bool, String> {
        let txt = std::fs::read_to_string(&import_path).map_err(|e| e.to_string())?;
        let bundle: Bundle =
            serde_json::from_str(&txt).map_err(|e| format!("not a valid backup file: {e}"))?;

        let dir = data_dir(&app)?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        // Snapshot current settings before overwriting so a mistaken restore (wrong
        // file) is recoverable by re-importing the snapshot. Best-effort: don't block
        // the restore if the snapshot can't be written.
        // ponytail: each snapshot holds plaintext SSH passphrases (0600) and is
        // never pruned, so secrets accumulate in the app data dir over many imports.
        // Accepted tradeoff: the files are owner-only and the export path already
        // writes secrets to disk. Prune to newest N or drop secrets from the
        // snapshot if the sprawl ever matters.
        if let Ok(snap) = build_bundle(&app) {
            if let Ok(json) = serde_json::to_string_pretty(&snap) {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let name = format!("{IMPORT_SNAPSHOT_PREFIX}-{ts}.json");
                let _ = write_secret_file(&dir.join(name), &json);
            }
        }

        for (name, val) in &bundle.files {
            // Trust boundary: the backup file is untrusted, so never let a crafted
            // file name escape the app data dir.
            if name.contains('/') || name.contains('\\') || name.contains("..") {
                continue;
            }
            // hosts.json feeds system ssh/scp (wpdeploy), so a crafted host could
            // carry an option-injecting alias/user/hostname the SSH UI would reject.
            // Validate imported hosts the same way host_save does; drop any that fail
            // rather than writing them verbatim.
            let val = if name == "hosts.json" {
                sanitized_hosts(val)?
            } else {
                val.clone()
            };
            let data = serde_json::to_string_pretty(&val).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(name), data).map_err(|e| e.to_string())?;
        }
        let mut failed = Vec::new();
        for (key, secret) in &bundle.secrets {
            if crate::ssh::keychain::set_passphrase(key, secret).is_err() {
                failed.push(key.clone());
            }
        }
        if !failed.is_empty() {
            // Configs are already written; report the partial restore instead of a
            // silent Ok so the user knows SSH passphrases are missing.
            return Err(format!(
                "Settings restored, but {} SSH passphrase(s) could not be saved to the keychain: {}",
                failed.len(),
                failed.join(", ")
            ));
        }
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}
