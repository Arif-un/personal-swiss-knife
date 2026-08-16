//! Tauri commands backing the `/deploy` page: manage the name list and
//! dispatch/track devkon deploys via the `gh` CLI.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::github::gh::{run_gh, run_gh_json};

use super::{mode_params, DevkonEntry, DevkonStore, RunStatus, REPO, WORKFLOW};

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("devkon.json"))
}

fn load(path: &Path) -> Result<DevkonStore, String> {
    if !path.exists() {
        return Ok(DevkonStore::default());
    }
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if data.trim().is_empty() {
        return Ok(DevkonStore::default());
    }
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

fn save(path: &Path, store: &DevkonStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    // Atomic write: temp + rename so a concurrent reader never sees a half-written
    // file. Lost updates between the several writers are prevented by update_store.
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

// ponytail: single global lock for the one devkon.json file; per-store locks would
// only matter if we ever managed multiple stores.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Locked read-modify-write of the store: serializes the several writers
/// (devkon_save, dispatch, status polls) so a concurrent load-mutate-save can't
/// clobber another's committed change. Keep `f` fast and network-free - the lock
/// is held for its whole duration.
fn update_store<T>(path: &Path, f: impl FnOnce(&mut DevkonStore) -> T) -> Result<T, String> {
    let _guard = STORE_LOCK.lock().unwrap();
    let mut store = load(path)?;
    let out = f(&mut store);
    save(path, &store)?;
    Ok(out)
}

/// Namespace names become k8s namespaces + DNS labels + workflow inputs, so hold
/// them to a strict RFC-1123 label (mirrors ssh validate_host rejecting unsafe
/// input). Blocks both broken deployments and any injection into the workflow.
fn validate_namespace(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 63
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(
            "name must be a valid namespace: lowercase letters, digits and dashes, \
             not starting or ending with a dash, max 63 chars"
                .into(),
        )
    }
}

/// A row from `gh run list --json databaseId,url`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunListItem {
    database_id: u64,
    #[serde(default)]
    url: String,
}

/// A run's state from `gh run view --json status,conclusion,updatedAt`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunView {
    #[serde(default)]
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    updated_at: String,
}

/// Newest workflow run on `branch`, or `None` if the branch has no runs.
/// Propagates gh errors (distinct from "no runs") so a failed lookup can't be
/// mistaken for an empty branch.
fn newest_run(branch: &str) -> Result<Option<RunListItem>, String> {
    let runs: Vec<RunListItem> = run_gh_json([
        "run",
        "list",
        "--workflow",
        WORKFLOW,
        "-R",
        REPO,
        "-b",
        branch,
        "-L",
        "1",
        "--json",
        "databaseId,url",
    ])?;
    Ok(runs.into_iter().next())
}

#[tauri::command]
pub fn devkon_list(window: WebviewWindow, app: AppHandle) -> Result<DevkonStore, String> {
    crate::security::require_main(&window)?;
    load(&store_path(&app)?)
}

/// Upsert an entry. A blank `id` creates a new one (assigns an id); an existing
/// `id` overwrites (rename / branch / mode changes). Returns the saved entry.
#[tauri::command]
pub fn devkon_save(
    window: WebviewWindow,
    app: AppHandle,
    mut entry: DevkonEntry,
) -> Result<DevkonEntry, String> {
    crate::security::require_main(&window)?;
    entry.name = entry.name.trim().to_string();
    validate_namespace(&entry.name)?;
    let path = store_path(&app)?;
    if entry.id.is_empty() {
        entry.id = format!("dk:{}", uuid::Uuid::new_v4());
    }
    update_store(&path, |store| {
        match store.entries.iter_mut().find(|e| e.id == entry.id) {
            // Preserve run tracking across edits: the UI only sends name/branch/mode.
            Some(existing) => {
                existing.name = entry.name.clone();
                existing.branch = entry.branch.clone();
                existing.mode = entry.mode.clone();
                existing.clone()
            }
            None => {
                store.entries.push(entry.clone());
                entry
            }
        }
    })
}

/// Remove a name from the list. Does not touch the cluster - use destroy first.
#[tauri::command]
pub fn devkon_delete(window: WebviewWindow, app: AppHandle, id: String) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = store_path(&app)?;
    update_store(&path, |store| store.entries.retain(|e| e.id != id))
}

/// All branch names of the deploy repo, for the per-row branch picker.
#[tauri::command]
pub async fn devkon_branches(window: WebviewWindow) -> Result<Vec<String>, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(|| {
        // per_page=100 + --paginate: fewer round-trips than the default 30/page.
        // ponytail: fetches every branch; fine for react-query-cached use, slow
        // only on a repo with thousands of branches.
        let path = format!("repos/{REPO}/branches?per_page=100");
        let out = run_gh(["api", path.as_str(), "--paginate", "-q", ".[].name"])?;
        let mut names: Vec<String> = out
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        names.sort();
        names.dedup();
        Ok::<_, String>(names)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn devkon_deploy(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<DevkonEntry, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, &id, "apply"))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn devkon_destroy(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<DevkonEntry, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || dispatch(&app, &id, "destroy"))
        .await
        .map_err(|e| e.to_string())?
}

/// Dispatch the workflow for `id` with `cmd` (`apply`/`destroy`), then capture
/// the created run id and persist it on the entry. Blocking (network + sleeps).
fn dispatch(app: &AppHandle, id: &str, cmd: &str) -> Result<DevkonEntry, String> {
    let path = store_path(app)?;
    let store = load(&path)?;
    let entry = store
        .entries
        .iter()
        .find(|e| e.id == id)
        .ok_or("unknown entry")?
        .clone();
    if entry.branch.trim().is_empty() {
        return Err("pick a branch first".into());
    }

    let (type_, clean) = mode_params(&entry.mode);
    // Baseline before dispatch. A gh failure here aborts before we trigger the
    // workflow, so we never mistake a pre-existing run for the one we launched.
    let before = newest_run(&entry.branch)?.map(|r| r.database_id);

    run_gh([
        "workflow",
        "run",
        WORKFLOW,
        "-R",
        REPO,
        "--ref",
        &entry.branch,
        "-f",
        &format!("namespace={}", entry.name),
        "-f",
        &format!("cmd={cmd}"),
        "-f",
        &format!("type={type_}"),
        "-f",
        &format!("clean={clean}"),
    ])?;

    // Watch for the run this dispatch created: newest on the ref whose id differs
    // from the pre-dispatch one. ponytail: two names sharing a branch dispatched
    // within the same poll window could grab each other's run - rare for a
    // single user; add a per-run marker input if it ever bites.
    let mut created: Option<RunListItem> = None;
    for _ in 0..6 {
        std::thread::sleep(Duration::from_secs(2));
        // Ignore transient poll errors; a later iteration (or a status poll) retries.
        if let Ok(Some(r)) = newest_run(&entry.branch) {
            if Some(r.database_id) != before {
                created = Some(r);
                break;
            }
        }
    }

    // Reload under the store lock: we held our read across the ~12s poll, during
    // which a concurrent devkon_save / status write may have changed the file.
    // update_store re-reads so we touch only this entry and never clobber those.
    update_store(&path, |store| {
        let entry = store
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or("unknown entry")?;
        entry.last_run_kind = Some(cmd.to_string());
        match created {
            Some(run) => {
                entry.last_run_id = Some(run.database_id);
                entry.last_run_url = Some(run.url);
                entry.baseline_run_id = None;
            }
            // Run didn't surface within the window: mark it "awaiting" (kind set, no
            // run id) and remember the baseline so a later status poll can adopt it.
            None => {
                entry.last_run_id = None;
                entry.last_run_url = None;
                entry.baseline_run_id = before;
            }
        }
        Ok::<DevkonEntry, String>(entry.clone())
    })?
}

/// Live status of a name's last tracked run. Also records the completion time of
/// a successful apply as the entry's `lastDeployedAt`.
#[tauri::command]
pub async fn devkon_status(
    window: WebviewWindow,
    app: AppHandle,
    id: String,
) -> Result<RunStatus, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || status_blocking(&app, &id))
        .await
        .map_err(|e| e.to_string())?
}

fn status_blocking(app: &AppHandle, id: &str) -> Result<RunStatus, String> {
    let path = store_path(app)?;
    let store = load(&path)?;
    let mut entry = store
        .entries
        .iter()
        .find(|e| e.id == id)
        .ok_or("unknown entry")?
        .clone();

    // Awaiting = a dispatch happened (kind set) but its run id was never captured
    // within the watch window. Try to adopt the first run newer than the baseline.
    if entry.last_run_id.is_none() && entry.last_run_kind.is_some() {
        if let Some(r) = newest_run(&entry.branch)? {
            if Some(r.database_id) != entry.baseline_run_id {
                entry.last_run_id = Some(r.database_id);
                entry.last_run_url = Some(r.url);
                let (rid, rurl) = (entry.last_run_id, entry.last_run_url.clone());
                update_store(&path, move |store| {
                    if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                        e.last_run_id = rid;
                        e.last_run_url = rurl;
                    }
                })?;
            }
        }
    }

    let Some(run_id) = entry.last_run_id else {
        return Ok(RunStatus {
            state: "none".into(),
            last_deployed_at: entry.last_deployed_at,
            ..Default::default()
        });
    };

    let view: RunView = run_gh_json([
        "run",
        "view",
        &run_id.to_string(),
        "-R",
        REPO,
        "--json",
        "status,conclusion,updatedAt",
    ])?;

    let mut last_deployed_at = entry.last_deployed_at.clone();
    if view.status == "completed"
        && view.conclusion.as_deref() == Some("success")
        && entry.last_run_kind.as_deref() == Some("apply")
    {
        last_deployed_at = Some(view.updated_at.clone());
        let lda = last_deployed_at.clone();
        update_store(&path, move |store| {
            if let Some(e) = store.entries.iter_mut().find(|e| e.id == id) {
                e.last_deployed_at = lda;
            }
        })?;
    }

    Ok(RunStatus {
        run_id: Some(run_id),
        kind: entry.last_run_kind,
        state: view.status,
        conclusion: view.conclusion,
        last_deployed_at,
    })
}
