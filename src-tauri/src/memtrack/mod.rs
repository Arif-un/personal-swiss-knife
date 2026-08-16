//! RAM-usage tracking for the app and the process tree it spawns.
//!
//! A background task samples resident memory (RSS) for the app process plus
//! every descendant it launched (helper webviews, the `gh` CLI, etc.) every
//! [`SAMPLE_INTERVAL_SECS`], persists each snapshot to SQLite, and caps the
//! stored history at [`MAX_SNAPSHOTS`] rows. The `/memory` page reads this history.
//!
//! Sampling only runs while the app is running; gaps in the history mark times
//! the app was closed (it can't measure itself when it isn't running).

pub mod commands;
mod sampler;
mod store;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sysinfo::System;
use tauri::{AppHandle, Manager};

const DB_FILE: &str = "memtrack.sqlite";
/// Snapshot cadence: every 15 minutes.
pub const SAMPLE_INTERVAL_SECS: u64 = 15 * 60;
/// Target history window: roughly the last 30 days.
pub const RETENTION_SECS: i64 = 30 * 24 * 60 * 60;
/// Hard cap on retained snapshot rows: ~30 days at the sample cadence plus
/// headroom for manual "Snapshot now" rows. A row cap (rather than a wall-clock
/// cutoff) keeps pruning immune to system-clock jumps. See [`store::prune`].
pub const MAX_SNAPSHOTS: i64 = RETENTION_SECS / SAMPLE_INTERVAL_SECS as i64 + 512;

/// RSS of a single process at snapshot time.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcSample {
    pub pid: u32,
    pub name: String,
    pub rss_bytes: u64,
    /// True for the app's own (root) process; false for spawned descendants.
    pub is_main: bool,
}

/// A full snapshot: the summed total plus the per-process breakdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Unix seconds.
    pub ts: i64,
    pub total_rss: u64,
    pub processes: Vec<ProcSample>,
}

/// Lightweight point for the time-series chart (no per-process detail).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSummary {
    /// Unix seconds.
    pub ts: i64,
    pub total_rss: u64,
}

/// Managed Tauri state: the SQLite connection shared by the sampler task and the
/// query commands (the `Mutex` serialises the infrequent writes against reads).
/// `None` means initialisation failed and memory tracking is disabled for this
/// run, so commands degrade gracefully instead of the whole app failing to start.
/// The `Arc` lets commands clone a handle into `spawn_blocking`, so the blocking
/// SQLite work never runs on the main thread or an async runtime worker.
pub struct MemStore(pub Arc<Mutex<Option<rusqlite::Connection>>>);

impl MemStore {
    /// A store with tracking disabled (DB unavailable): commands return empty
    /// results and the sampler is not spawned.
    pub fn disabled() -> Self {
        MemStore(Arc::new(Mutex::new(None)))
    }
}

/// Open (creating if needed) the history database under the app data dir.
pub fn init(app: &AppHandle) -> Result<MemStore, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let conn = store::open(&dir.join(DB_FILE))?;
    Ok(MemStore(Arc::new(Mutex::new(Some(conn)))))
}

/// Spawn the background sampler. The first tick fires immediately, so a snapshot
/// is recorded at launch and then every [`SAMPLE_INTERVAL_SECS`]. The blocking
/// process scan and SQLite write run on the blocking pool so they never stall an
/// async runtime worker.
pub fn spawn_sampler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(SAMPLE_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let app = app.clone();
            if let Err(e) = tauri::async_runtime::spawn_blocking(move || {
                let mut sys = System::new();
                let snap = sampler::sample(&mut sys, &app);
                persist(&app, &snap);
            })
            .await
            {
                // A panic in the blocking scan/persist must not permanently kill
                // tracking; log and re-arm on the next tick instead of returning.
                eprintln!("memtrack: sampler task failed, retrying next tick: {e}");
            }
        }
    });
}

/// Insert a snapshot and prune old rows, logging (not propagating) failures so a
/// transient DB error never kills the sampler loop. A no-op when tracking is
/// disabled.
fn persist(app: &AppHandle, snap: &Snapshot) {
    let state = app.state::<MemStore>();
    let guard = match state.0.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("memtrack: store lock poisoned: {e}");
            return;
        }
    };
    let Some(conn) = guard.as_ref() else {
        return;
    };
    if let Err(e) = store::insert(conn, snap).and_then(|_| store::prune(conn, MAX_SNAPSHOTS)) {
        eprintln!("memtrack: failed to persist snapshot: {e}");
    }
}
