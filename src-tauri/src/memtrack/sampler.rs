//! Walks the app's process tree and reads each process's resident memory.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessesToUpdate, System};

use super::{ProcSample, Snapshot};

#[cfg(target_os = "macos")]
extern "C" {
    /// Private but long-stable libSystem call (the one Activity Monitor uses) that
    /// maps a pid to the pid ultimately responsible for it. macOS reparents WKWebView
    /// helper processes (the `WebContent` renderer that actually holds the Messenger
    /// page's RAM, plus its Networking/GPU siblings) to `launchd`, so they escape a
    /// parent-tree walk — but their responsible pid still points back at us.
    /// Returns the responsible pid, or <= 0 on error.
    fn responsibility_get_pid_responsible_for_pid(pid: i32) -> i32;
}

/// Take one snapshot: refresh process info, collect the app process plus every
/// descendant it spawned, and sum their RSS.
///
/// On macOS, WebKit helper processes are reparented to `launchd` and so escape
/// the parent-child walk; they're recovered here via the responsible-pid mapping
/// so webview (Messenger) memory is counted.
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

    // Recover WebKit helpers macOS reparented to launchd: any process whose
    // "responsible" pid lands inside our tree is really ours. Checked against the
    // pre-existing tree so a helper can only attach to us, never transitively to
    // another helper. Cheap syscall per not-yet-seen process; macOS-only.
    #[cfg(target_os = "macos")]
    {
        let recovered: Vec<Pid> = sys
            .processes()
            .keys()
            .copied()
            .filter(|pid| !tree.contains(pid))
            .filter(|pid| {
                let rpid =
                    unsafe { responsibility_get_pid_responsible_for_pid(pid.as_u32() as i32) };
                rpid > 0 && tree.contains(&Pid::from_u32(rpid as u32))
            })
            .collect();
        tree.extend(recovered);
    }

    let mut processes: Vec<ProcSample> = tree
        .iter()
        .filter_map(|pid| sys.process(*pid))
        .map(|p| ProcSample {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            rss_bytes: p.memory(),
            is_main: p.pid() == own,
        })
        .collect();
    // Main process pinned first, then descendants by RSS descending.
    processes.sort_by(|a, b| {
        b.is_main
            .cmp(&a.is_main)
            .then(b.rss_bytes.cmp(&a.rss_bytes))
    });

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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// Guards the extern's symbol name and ABI, and the semantics the fix relies
    /// on: the call resolves a pid to its top-level responsible process, which is
    /// responsible for itself — so it's a fixed point (`f(f(x)) == f(x) > 0`).
    /// That fixed point is our own app when it hosts a WKWebView, which is exactly
    /// what a reparented WebContent helper resolves back to. If this breaks,
    /// WebKit-helper recovery silently attributes nothing.
    #[test]
    fn responsible_pid_is_a_valid_fixed_point() {
        let me = std::process::id() as i32;
        let top = unsafe { responsibility_get_pid_responsible_for_pid(me) };
        assert!(top > 0, "no responsible pid for self");
        let top2 = unsafe { responsibility_get_pid_responsible_for_pid(top) };
        assert_eq!(top, top2, "responsible pid must be responsible for itself");
    }
}
