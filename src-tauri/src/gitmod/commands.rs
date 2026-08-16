//! Tauri commands backing the `/submodules` page: persist the superproject
//! path, gather the parent + submodule rows (fetching remotes first), and
//! switch a repo's branch.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, WebviewWindow};

use super::git::{
    branch_list, fetch_and_pull, git, read_row, safe_arg, submodule_paths, switch_branch,
};
use super::{GitmodConfig, RepoRow, SwitchAllResult};

/// Target branches for "switch all", in preference order.
const SWITCH_ALL_TARGETS: [&str; 3] = ["develop", "master", "main"];

/// (dir, display name, is_parent) for the parent and each submodule under `root`.
fn repo_targets(root: &Path) -> Result<Vec<(PathBuf, String, bool)>, String> {
    let mut targets: Vec<(PathBuf, String, bool)> = vec![(root.to_path_buf(), ".".into(), true)];
    for s in submodule_paths(root)? {
        let path = root.join(&s);
        targets.push((path, s, false));
    }
    Ok(targets)
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("gitmod.json"))
}

fn load_config(app: &AppHandle) -> Result<GitmodConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(GitmodConfig::default());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if data.trim().is_empty() {
        return Ok(GitmodConfig::default());
    }
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gitmod_get_config(window: WebviewWindow, app: AppHandle) -> Result<GitmodConfig, String> {
    crate::security::require_main(&window)?;
    load_config(&app)
}

/// Persist the superproject path after validating it's a git repo.
#[tauri::command]
pub fn gitmod_set_config(
    window: WebviewWindow,
    app: AppHandle,
    path: String,
) -> Result<GitmodConfig, String> {
    crate::security::require_main(&window)?;
    let trimmed = path.trim().to_string();
    if !trimmed.is_empty() {
        let dir = Path::new(&trimmed);
        if !dir.is_dir() {
            return Err("path is not a directory".into());
        }
        // Must be a git work tree (else every later command fails cryptically).
        git(dir, ["rev-parse", "--is-inside-work-tree"])
            .map_err(|_| "path is not a git repository".to_string())?;
    }
    let cfg = GitmodConfig { path: trimmed };
    let file = config_path(&app)?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&file, data).map_err(|e| e.to_string())?;
    Ok(cfg)
}

/// Fetch remotes for the parent + every submodule (in parallel), then read each
/// repo's row. A per-repo fetch failure is recorded on that row; the page still
/// renders with local info.
#[tauri::command]
pub async fn gitmod_status(window: WebviewWindow, path: String) -> Result<Vec<RepoRow>, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(path.trim());
        if !root.is_dir() {
            return Err("path is not a directory".to_string());
        }
        let targets = repo_targets(&root)?;

        // Fetch every repo concurrently, then read its row. Reads are local and
        // fast; fetches are the slow network part, so we parallelize the whole
        // fetch+read per repo. Preserves target order.
        let rows = std::thread::scope(|scope| {
            let handles: Vec<_> = targets
                .into_iter()
                .map(|(dir, name, is_parent)| {
                    scope.spawn(move || {
                        let fetch_err = git(&dir, ["fetch", "--all", "--prune"]).err();
                        let mut row = read_row(&dir, name, is_parent);
                        // Keep the fetch error only if reading didn't already
                        // fail (a read error is the more useful message).
                        if row.error.is_none() {
                            row.error = fetch_err;
                        }
                        row
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        });
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Switch a repo's branch. `sub` is empty for the parent, else the submodule
/// path. `action` is `"none"` | `"stash"` | `"carry"` (dirty-tree strategy).
/// Returns the refreshed row for that repo.
#[tauri::command]
pub async fn gitmod_switch(
    window: WebviewWindow,
    path: String,
    sub: String,
    branch: String,
    action: String,
) -> Result<RepoRow, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(path.trim());
        let (dir, name, is_parent) = if sub.trim().is_empty() {
            (root.clone(), ".".to_string(), true)
        } else {
            safe_arg(sub.trim())?;
            (root.join(sub.trim()), sub.trim().to_string(), false)
        };
        if !dir.is_dir() {
            return Err("repo path not found".to_string());
        }
        switch_branch(&dir, branch.trim(), action.trim())?;
        Ok(read_row(&dir, name, is_parent))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Open a repo (parent or submodule) in an external app. `sub` is empty for the
/// parent, else the submodule path. `app` is `"github"`, `"vscode"` or
/// `"terminal"`. macOS only (`open -a <AppName>`).
#[tauri::command]
pub fn gitmod_open_app(
    window: WebviewWindow,
    path: String,
    sub: String,
    app: String,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let app_name = match app.trim() {
        "github" => "GitHub Desktop",
        "vscode" => "Visual Studio Code",
        "terminal" => "Terminal",
        other => return Err(format!("unknown app: {other}")),
    };
    let root = PathBuf::from(path.trim());
    let dir = if sub.trim().is_empty() {
        root
    } else {
        safe_arg(sub.trim())?;
        root.join(sub.trim())
    };
    if !dir.is_dir() {
        return Err("repo path not found".to_string());
    }
    let status = std::process::Command::new("open")
        .args(["-a", app_name])
        .arg(&dir)
        .status()
        .map_err(|e| format!("failed to launch {app_name}: {e}"))?;
    if !status.success() {
        return Err(format!("{app_name} is not installed"));
    }
    Ok(())
}

/// Fetch + fast-forward the parent and every submodule (in parallel), then read
/// each row. Like `gitmod_status` but also `git pull --ff-only` on repos that
/// track an upstream. Per-repo failures are recorded on that row's error.
#[tauri::command]
pub async fn gitmod_refresh_pull(
    window: WebviewWindow,
    path: String,
) -> Result<Vec<RepoRow>, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(path.trim());
        if !root.is_dir() {
            return Err("path is not a directory".to_string());
        }
        let targets = repo_targets(&root)?;
        let rows = std::thread::scope(|scope| {
            let handles: Vec<_> = targets
                .into_iter()
                .map(|(dir, name, is_parent)| {
                    scope.spawn(move || {
                        let pull_err = fetch_and_pull(&dir);
                        let mut row = read_row(&dir, name, is_parent);
                        if row.error.is_none() {
                            row.error = pull_err;
                        }
                        row
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        });
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Switch the parent and every submodule to the first existing branch of
/// `develop`/`master`/`main` (in that order). `action` is the dirty-tree
/// strategy applied to every dirty repo (`"stash"` | `"carry"`). Repos with
/// none of the target branches are left as-is and reported in `notes`.
#[tauri::command]
pub async fn gitmod_switch_all(
    window: WebviewWindow,
    path: String,
    action: String,
) -> Result<SwitchAllResult, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(path.trim());
        if !root.is_dir() {
            return Err("path is not a directory".to_string());
        }
        let targets = repo_targets(&root)?;
        let action = action.trim().to_string();

        // (row, optional note) per repo, run concurrently and re-collected.
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = targets
                .into_iter()
                .map(|(dir, name, is_parent)| {
                    let action = action.clone();
                    scope.spawn(move || switch_one(&dir, name, is_parent, &action))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        });

        let mut rows = Vec::with_capacity(results.len());
        let mut notes = Vec::new();
        for (row, note) in results {
            if let Some(n) = note {
                notes.push(n);
            }
            rows.push(row);
        }
        Ok(SwitchAllResult { rows, notes })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Switch one repo to its preferred target branch. Returns its refreshed row and
/// a note when skipped (no target branch) or the switch failed.
fn switch_one(
    dir: &Path,
    name: String,
    is_parent: bool,
    action: &str,
) -> (RepoRow, Option<String>) {
    let label = if is_parent { "parent" } else { name.as_str() };
    let branches = branch_list(dir);
    let target = SWITCH_ALL_TARGETS
        .iter()
        .find(|t| branches.iter().any(|b| b == *t));
    let Some(&target) = target else {
        return (
            read_row(dir, name.clone(), is_parent),
            Some(format!("{label}: no develop/master/main branch")),
        );
    };
    // Already on it? Nothing to do.
    let current = git(dir, ["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if current == target {
        return (read_row(dir, name, is_parent), None);
    }
    let note = switch_branch(dir, target, action)
        .err()
        .map(|e| format!("{label}: {e}"));
    (read_row(dir, name, is_parent), note)
}
