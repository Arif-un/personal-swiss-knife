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
            rss         INTEGER NOT NULL,
            is_main     INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_snapshots_ts ON snapshots(ts);
        CREATE INDEX IF NOT EXISTS idx_proc_snapshot ON proc_samples(snapshot_id);",
    )
    .map_err(|e| e.to_string())?;

    // Migrate DBs created before the is_main column existed. Old rows keep 0
    // (no main flagged) which is harmless: their table just shows no "main" badge.
    let has_is_main = conn
        .prepare("SELECT 1 FROM pragma_table_info('proc_samples') WHERE name = 'is_main'")
        .and_then(|mut s| s.exists([]))
        .map_err(|e| e.to_string())?;
    if !has_is_main {
        conn.execute(
            "ALTER TABLE proc_samples ADD COLUMN is_main INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
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
            .prepare(
                "INSERT INTO proc_samples (snapshot_id, pid, name, rss, is_main) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| e.to_string())?;
        for p in &snap.processes {
            stmt.execute(params![id, p.pid, p.name, p.rss_bytes, p.is_main])
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
            head_row,
        )
        .optional()
        .map_err(|e| e.to_string())?;
    hydrate(conn, head)
}

/// The snapshot recorded at `ts` (the newest one if several share that second),
/// with its per-process breakdown, or `None` if there is none at that time.
pub fn snapshot_at(conn: &Connection, ts: i64) -> Result<Option<Snapshot>, String> {
    let head = conn
        .query_row(
            "SELECT id, ts, total_rss FROM snapshots WHERE ts = ?1 ORDER BY id DESC LIMIT 1",
            params![ts],
            head_row,
        )
        .optional()
        .map_err(|e| e.to_string())?;
    hydrate(conn, head)
}

/// Row mapper for the `(id, ts, total_rss)` snapshot header tuple.
fn head_row(row: &rusqlite::Row) -> rusqlite::Result<(i64, i64, u64)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

/// Load the per-process rows for a snapshot header into a full `Snapshot`.
fn hydrate(conn: &Connection, head: Option<(i64, i64, u64)>) -> Result<Option<Snapshot>, String> {
    let Some((id, ts, total_rss)) = head else {
        return Ok(None);
    };

    let mut stmt = conn
        .prepare(
            "SELECT pid, name, rss, is_main FROM proc_samples \
             WHERE snapshot_id = ?1 ORDER BY is_main DESC, rss DESC",
        )
        .map_err(|e| e.to_string())?;
    let processes = stmt
        .query_map(params![id], |row| {
            Ok(ProcSample {
                pid: row.get(0)?,
                name: row.get(1)?,
                rss_bytes: row.get(2)?,
                is_main: row.get(3)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(ts: i64) -> Snapshot {
        Snapshot {
            ts,
            total_rss: 300,
            processes: vec![
                // main deliberately has the smaller RSS to prove it's pinned first
                // by is_main, not by size.
                ProcSample {
                    pid: 1,
                    name: "app".into(),
                    rss_bytes: 100,
                    is_main: true,
                },
                ProcSample {
                    pid: 2,
                    name: "helper".into(),
                    rss_bytes: 200,
                    is_main: false,
                },
            ],
        }
    }

    #[test]
    fn snapshot_at_roundtrips_main_first() {
        let conn = open(std::path::Path::new(":memory:")).unwrap();
        insert(&conn, &snap(1000)).unwrap();
        insert(&conn, &snap(2000)).unwrap();

        let got = snapshot_at(&conn, 1000).unwrap().expect("snapshot at 1000");
        assert_eq!(got.ts, 1000);
        assert_eq!(got.processes.len(), 2);
        assert!(got.processes[0].is_main && got.processes[0].pid == 1);
        assert!(!got.processes[1].is_main);

        assert!(snapshot_at(&conn, 999).unwrap().is_none());
        assert_eq!(latest(&conn).unwrap().unwrap().ts, 2000);
    }
}
