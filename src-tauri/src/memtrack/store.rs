//! SQLite persistence for RAM snapshots.

use rusqlite::{params, Connection, OptionalExtension};

use super::{ProcSample, Snapshot, SnapshotSummary};

/// Open the database and ensure the schema exists.
pub fn open(path: &std::path::Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS snapshots (
            id        INTEGER PRIMARY KEY,
            ts        INTEGER NOT NULL,
            total_rss INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS proc_samples (
            snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
            pid         INTEGER NOT NULL,
            name        TEXT    NOT NULL,
            rss         INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_snapshots_ts ON snapshots(ts);
        CREATE INDEX IF NOT EXISTS idx_proc_snapshot ON proc_samples(snapshot_id);",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Persist one snapshot and its per-process rows atomically, so a mid-batch
/// failure never leaves a snapshot whose `total_rss` disagrees with a truncated
/// process list.
pub fn insert(conn: &Connection, snap: &Snapshot) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO snapshots (ts, total_rss) VALUES (?1, ?2)",
        params![snap.ts, snap.total_rss],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();

    {
        let mut stmt = tx
            .prepare("INSERT INTO proc_samples (snapshot_id, pid, name, rss) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|e| e.to_string())?;
        for p in &snap.processes {
            stmt.execute(params![id, p.pid, p.name, p.rss_bytes])
                .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Time-ordered summaries (ts + total) with `ts >= since`, oldest first.
pub fn history(conn: &Connection, since: i64) -> Result<Vec<SnapshotSummary>, String> {
    let mut stmt = conn
        .prepare("SELECT ts, total_rss FROM snapshots WHERE ts >= ?1 ORDER BY ts ASC, id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![since], |row| {
            Ok(SnapshotSummary {
                ts: row.get(0)?,
                total_rss: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Most recent snapshot with its per-process breakdown, or `None` if empty.
pub fn latest(conn: &Connection) -> Result<Option<Snapshot>, String> {
    let head = conn
        .query_row(
            "SELECT id, ts, total_rss FROM snapshots ORDER BY ts DESC, id DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, u64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let Some((id, ts, total_rss)) = head else {
        return Ok(None);
    };

    let mut stmt = conn
        .prepare("SELECT pid, name, rss FROM proc_samples WHERE snapshot_id = ?1 ORDER BY rss DESC")
        .map_err(|e| e.to_string())?;
    let processes = stmt
        .query_map(params![id], |row| {
            Ok(ProcSample {
                pid: row.get(0)?,
                name: row.get(1)?,
                rss_bytes: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(Some(Snapshot {
        ts,
        total_rss,
        processes,
    }))
}

/// Keep only the most recent `keep` snapshots (proc_samples cascade). Row-count
/// based rather than timestamp based, so a wild system-clock jump can't push a
/// retention cutoff past every real row and wipe the whole history.
pub fn prune(conn: &Connection, keep: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM snapshots
         WHERE id NOT IN (SELECT id FROM snapshots ORDER BY ts DESC, id DESC LIMIT ?1)",
        params![keep],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
