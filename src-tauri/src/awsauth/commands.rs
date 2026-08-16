//! Tauri commands backing the `/deploy` page's AWS login button.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use tauri::{AppHandle, Manager, WebviewWindow};

use super::{AwsAuthConfig, AWSAUTH_REL, CREDENTIALS_REL, LOGIN_URL};

/// How long to wait for the Docker daemon to come up after launching it. Cold
/// starts of Docker Desktop can take well over a minute on first boot.
const DOCKER_WAIT: Duration = Duration::from_secs(120);
const BRAVE_BIN: &str = "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser";

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("awsauth.json"))
}

fn load(path: &Path) -> AwsAuthConfig {
    // Missing / empty / corrupt config falls back to defaults - this is a
    // convenience helper, not data worth failing over.
    std::fs::read_to_string(path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

#[tauri::command]
pub fn awsauth_get_config(window: WebviewWindow, app: AppHandle) -> Result<AwsAuthConfig, String> {
    crate::security::require_main(&window)?;
    Ok(load(&config_path(&app)?))
}

#[tauri::command]
pub fn awsauth_set_config(
    window: WebviewWindow,
    app: AppHandle,
    config: AwsAuthConfig,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = config_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

/// Step 1: validate config, capture the credentials file's current mtime as the
/// freshness baseline, then open Brave. Returns the baseline (epoch millis, or
/// null if the file doesn't exist yet) for the frontend to poll with
/// [`awsauth_check_fresh`]. The countdown/cancel loop lives in the frontend, so
/// no blocking wait happens here.
#[tauri::command]
pub fn awsauth_open_brave(window: WebviewWindow, app: AppHandle) -> Result<Option<u64>, String> {
    crate::security::require_main(&window)?;
    let cfg = load(&config_path(&app)?);
    let repo_dir = PathBuf::from(cfg.repo_dir.trim());
    if !repo_dir.join(AWSAUTH_REL).is_file() {
        return Err(format!(
            "awsauth script not found at {}. Set the repo directory correctly.",
            repo_dir.join(AWSAUTH_REL).display()
        ));
    }
    let profile_dir = resolve_brave_profile_dir(cfg.brave_profile.trim())?;
    let baseline = mtime_millis(&home()?.join(CREDENTIALS_REL));
    open_brave(&profile_dir)?;
    Ok(baseline)
}

/// Step 2 (polled during the countdown): true once the credentials file is newer
/// than `baseline` (or exists, if it didn't at step 1) - i.e. the manual download
/// landed.
#[tauri::command]
pub fn awsauth_check_fresh(window: WebviewWindow, baseline: Option<u64>) -> Result<bool, String> {
    crate::security::require_main(&window)?;
    let cur = mtime_millis(&home()?.join(CREDENTIALS_REL));
    Ok(match baseline {
        Some(b) => cur.is_some_and(|c| c > b),
        None => cur.is_some(),
    })
}

/// Step 3: once the credentials file is fresh, ensure Docker is up and run
/// `tools/awsauth`. Blocking (up to 120s Docker poll + the subprocess), so it runs
/// off the async runtime.
#[tauri::command]
pub async fn awsauth_finish(window: WebviewWindow, app: AppHandle) -> Result<String, String> {
    crate::security::require_main(&window)?;
    let cfg = load(&config_path(&app)?);
    tauri::async_runtime::spawn_blocking(move || {
        let repo_dir = PathBuf::from(cfg.repo_dir.trim());
        ensure_docker()?;
        run_awsauth(&repo_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Map a Brave profile *display* name to its on-disk directory (e.g. `OP` ->
/// `Default`) via Brave's `Local State`. Falls back to the given value so a raw
/// directory name (or a missing Local State) still works.
fn resolve_brave_profile_dir(display: &str) -> Result<String, String> {
    if display.is_empty() {
        return Err("Brave profile name is empty".into());
    }
    let ls = home()?.join("Library/Application Support/BraveSoftware/Brave-Browser/Local State");
    let Ok(data) = std::fs::read_to_string(&ls) else {
        return Ok(display.to_string());
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
        return Ok(display.to_string());
    };
    if let Some(cache) = json
        .get("profile")
        .and_then(|p| p.get("info_cache"))
        .and_then(|c| c.as_object())
    {
        for (dir, info) in cache {
            if info.get("name").and_then(|n| n.as_str()) == Some(display) {
                return Ok(dir.clone());
            }
        }
    }
    // No display-name match: assume the user typed the directory name directly.
    Ok(display.to_string())
}

/// Open the login URL twice (two tabs) in the chosen Brave profile. Launching the
/// binary directly targets the profile and forwards the URLs to a running Brave.
fn open_brave(profile_dir: &str) -> Result<(), String> {
    if !Path::new(BRAVE_BIN).exists() {
        return Err(format!("Brave not found at {BRAVE_BIN}"));
    }
    Command::new(BRAVE_BIN)
        .arg(format!("--profile-directory={profile_dir}"))
        .arg(LOGIN_URL)
        .arg(LOGIN_URL)
        .spawn()
        .map_err(|e| format!("failed to launch Brave: {e}"))?;
    Ok(())
}

/// The file's mtime as epoch millis, or None if it doesn't exist / is unreadable.
fn mtime_millis(path: &Path) -> Option<u64> {
    let t = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// True if the Docker daemon answers. Connects directly to the Docker Desktop
/// socket first: that's PATH- and binary-independent, so it works from the GUI
/// app's minimal environment where `docker` may not resolve on PATH (the reason
/// the old `docker info` check could time out even with Docker running). Falls
/// back to `docker info` through a login shell for non-default contexts (e.g. a
/// remote DOCKER_HOST).
fn docker_running() -> bool {
    if docker_sockets()
        .iter()
        .any(|s| UnixStream::connect(s).is_ok())
    {
        return true;
    }
    Command::new("/bin/zsh")
        .args(["-lc", "docker info >/dev/null 2>&1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Candidate Docker Desktop daemon sockets, most specific first.
fn docker_sockets() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(h) = home() {
        v.push(h.join(".docker/run/docker.sock"));
    }
    v.push(PathBuf::from("/var/run/docker.sock"));
    v
}

/// Ensure the Docker daemon is up: if not, launch Docker Desktop and poll until
/// it answers or the wait window elapses.
fn ensure_docker() -> Result<(), String> {
    if docker_running() {
        return Ok(());
    }
    Command::new("/usr/bin/open")
        .args(["-a", "Docker"])
        .status()
        .map_err(|e| format!("failed to launch Docker: {e}"))?;
    let deadline = std::time::Instant::now() + DOCKER_WAIT;
    loop {
        std::thread::sleep(Duration::from_secs(2));
        if docker_running() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "Docker did not start within {}s",
                DOCKER_WAIT.as_secs()
            ));
        }
    }
}

/// Run `tools/awsauth` from the repo root via a login shell (for PATH). Success is
/// the script's own exit status; the combined stdout+stderr is returned either way
/// so the UI can show what happened. (Keyed off the exit code, not a "Login
/// Succeeded" banner: a successful run whose `docker login` is a no-op - creds
/// already cached - prints no banner and must not read as a failure.)
fn run_awsauth(repo_dir: &Path) -> Result<String, String> {
    let out = Command::new("/bin/zsh")
        .args(["-lc", AWSAUTH_REL])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| format!("failed to run awsauth: {e}"))?;

    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    let combined = combined.trim().to_string();

    if out.status.success() {
        Ok(combined)
    } else {
        Err(format!(
            "awsauth failed (exit {}):\n{combined}",
            out.status.code().unwrap_or(-1)
        ))
    }
}
