//! Tauri commands backing the `/memory` page.

use sysinfo::System;
use tauri::State;

use super::{sampler, store, MemStore, Snapshot, SnapshotSummary, MAX_SNAPSHOTS};

/// All retained snapshot summaries for the chart (UI slices by range). Empty when
/// tracking is disabled. Async + `spawn_blocking` so the SQLite read never blocks
/// the main thread while contending with the sampler's write lock.
#[tauri::command]
pub async fn memory_history(
    window: tauri::WebviewWindow,
    store: State<'_, MemStore>,
) -> Result<Vec<SnapshotSummary>, String> {
    crate::security::require_main(&window)?;
    let conn = store.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let Some(conn) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        store::history(conn, 0)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The latest snapshot with per-process breakdown, or `None` when there is no
/// snapshot yet or tracking is disabled.
#[tauri::command]
pub async fn memory_latest(
    window: tauri::WebviewWindow,
    store: State<'_, MemStore>,
) -> Result<Option<Snapshot>, String> {
    crate::security::require_main(&window)?;
    let conn = store.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let Some(conn) = guard.as_ref() else {
            return Ok(None);
        };
        store::latest(conn)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The snapshot recorded at `ts` (unix seconds) with its per-process breakdown,
/// backing the chart's per-point drill-down. `None` when no snapshot matches or
/// tracking is disabled.
#[tauri::command]
pub async fn memory_snapshot_at(
    window: tauri::WebviewWindow,
    store: State<'_, MemStore>,
    ts: i64,
) -> Result<Option<Snapshot>, String> {
    crate::security::require_main(&window)?;
    let conn = store.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let Some(conn) = guard.as_ref() else {
            return Ok(None);
        };
        store::snapshot_at(conn, ts)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Take, persist, and return a snapshot on demand (the "Snapshot now" button).
/// The scan, insert, and prune all run on the blocking pool so neither the UI
/// thread nor an async runtime worker stalls on SQLite.
#[tauri::command]
pub async fn memory_snapshot_now(
    window: tauri::WebviewWindow,
    store: State<'_, MemStore>,
) -> Result<Snapshot, String> {
    crate::security::require_main(&window)?;
    let conn = store.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sys = System::new();
        let snap = sampler::sample(&mut sys);
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let Some(conn) = guard.as_ref() else {
            return Err("memory tracking is unavailable".into());
        };
        store::insert(conn, &snap)?;
        store::prune(conn, MAX_SNAPSHOTS)?;
        Ok(snap)
    })
    .await
    .map_err(|e| e.to_string())?
}
