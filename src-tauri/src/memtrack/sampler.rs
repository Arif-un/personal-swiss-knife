//! Walks the app's process tree and reads each process's resident memory.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessesToUpdate, System};

use super::{ProcSample, Snapshot};

/// Take one snapshot: refresh process info, collect the app process plus every
/// descendant it spawned, and sum their RSS.
///
/// Scope note: on macOS some WebKit helper processes are reparented to `launchd`
/// rather than staying children of the app, so a snapshot is best-effort for
/// those and may undercount webview memory.
pub fn sample(sys: &mut System) -> Snapshot {
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let own = Pid::from_u32(std::process::id());

    // parent -> children adjacency, so we can BFS the whole descendant tree.
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, proc_) in sys.processes() {
        if let Some(parent) = proc_.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }

    let mut tree: HashSet<Pid> = HashSet::new();
    let mut queue = vec![own];
    while let Some(pid) = queue.pop() {
        if !tree.insert(pid) {
            continue; // already visited (guards against any parent cycle)
        }
        if let Some(kids) = children.get(&pid) {
            queue.extend(kids.iter().copied());
        }
    }

    let mut processes: Vec<ProcSample> = tree
        .iter()
        .filter_map(|pid| sys.process(*pid))
        .map(|p| ProcSample {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            rss_bytes: p.memory(),
        })
        .collect();
    processes.sort_by_key(|p| std::cmp::Reverse(p.rss_bytes));

    let total_rss = processes.iter().map(|p| p.rss_bytes).sum();

    Snapshot {
        ts: now_unix(),
        total_rss,
        processes,
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
