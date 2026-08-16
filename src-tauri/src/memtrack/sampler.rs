//! Walks the app's process tree and reads each process's resident memory.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Manager};

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

/// Ask each of the app's own webviews for the pid of the WebKit `WebContent`
/// process that actually renders it, keyed pid -> a human label (`"<webview> —
/// <host>"`). This is the deterministic counterpart to the responsible-pid
/// heuristic: because the app created these webviews, `WKWebView` hands us the
/// exact renderer pid, so a webview's RAM is always counted and can be labelled
/// with which webview it belongs to.
///
/// `_webProcessIdentifier` is a private but long-stable `WKWebView` selector
/// (returns 0 before the renderer has spawned). The read must run on the UI
/// thread, so it's dispatched via `with_webview` and the result returned over a
/// channel with a short timeout, so a busy main thread can never stall sampling.
#[cfg(target_os = "macos")]
fn webview_pids(app: &AppHandle) -> HashMap<Pid, String> {
    use std::sync::mpsc;
    use std::time::Duration;

    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let mut out = HashMap::new();
    for (label, webview) in app.webviews() {
        // Host (if the URL is known) gives the row a recognisable name, e.g.
        // "messenger — www.facebook.com" rather than an opaque "WebContent".
        let host = webview
            .url()
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned));
        let display = match host {
            Some(h) => format!("{label} — {h}"),
            None => label,
        };

        let (tx, rx) = mpsc::channel();
        if webview
            .with_webview(move |pw| {
                let view = pw.inner() as *mut AnyObject;
                let pid: i32 = unsafe { msg_send![view, _webProcessIdentifier] };
                let _ = tx.send(pid);
            })
            .is_err()
        {
            continue;
        }
        if let Ok(pid) = rx.recv_timeout(Duration::from_millis(500)) {
            if pid > 0 {
                out.insert(Pid::from_u32(pid as u32), display);
            }
        }
    }
    out
}

/// Take one snapshot: refresh process info, collect the app process plus every
/// descendant it spawned, and sum their RSS.
///
/// On macOS, WebKit helper processes are reparented to `launchd` and so escape
/// the parent-child walk. They're recovered two ways: the app's webviews report
/// their exact renderer pid via [`webview_pids`] (deterministic, and labelled),
/// and any remaining sibling helper (GPU/Networking) is picked up via the
/// responsible-pid mapping so all webview memory is counted.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub fn sample(sys: &mut System, app: &AppHandle) -> Snapshot {
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

    // The app's own webviews report their exact renderer pid, so their RAM is
    // always counted and each row can be labelled with the webview it renders.
    #[cfg(target_os = "macos")]
    let labels = webview_pids(app);
    #[cfg(not(target_os = "macos"))]
    let labels: HashMap<Pid, String> = HashMap::new();
    tree.extend(labels.keys().copied());

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

    // ponytail: temporary diagnostic. Run with MEMTRACK_DEBUG=1 to dump every
    // process sysinfo sees, its RSS, its responsible-pid, and whether it got
    // counted — pinpoints why a helper (e.g. facebook.com WebContent) escapes.
    // Remove once the missing-process bug is understood.
    #[cfg(target_os = "macos")]
    if std::env::var_os("MEMTRACK_DEBUG").is_some() {
        let mut rows: Vec<(u32, u64, i32, bool, String)> = sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let rpid =
                    unsafe { responsibility_get_pid_responsible_for_pid(pid.as_u32() as i32) };
                (
                    pid.as_u32(),
                    p.memory(),
                    rpid,
                    tree.contains(pid),
                    p.name().to_string_lossy().into_owned(),
                )
            })
            .collect();
        rows.sort_by_key(|b| std::cmp::Reverse(b.1));
        eprintln!(
            "=== MEMTRACK_DEBUG own={} ({} procs) ===",
            own.as_u32(),
            rows.len()
        );
        for (pid, rss, rpid, in_tree, name) in rows.iter().take(40) {
            eprintln!(
                "{:>7} rss={:>6}MB rpid={:>7} {} {}",
                pid,
                rss / 1_048_576,
                rpid,
                if *in_tree { "IN " } else { "out" },
                name
            );
        }
    }

    let mut processes: Vec<ProcSample> = tree
        .iter()
        .filter_map(|pid| sys.process(*pid))
        .map(|p| ProcSample {
            pid: p.pid().as_u32(),
            // Prefer the friendly webview label ("messenger — www.facebook.com")
            // over the opaque OS process name ("com.apple.WebKit.WebContent").
            name: labels
                .get(&p.pid())
                .cloned()
                .unwrap_or_else(|| p.name().to_string_lossy().into_owned()),
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
