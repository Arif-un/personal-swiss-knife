//! Thin `git` CLI wrappers for the submodule dashboard. Everything runs via
//! `std::process::Command` with explicit args (no shell), so branch/path values
//! can't inject a shell command; we still reject leading-`-` values so they
//! can't be read as git options.

use std::path::Path;
use std::process::Command;

use super::RepoRow;

/// Run `git <args>` in `dir`, returning trimmed stdout. Non-zero exit -> Err
/// with stderr (or stdout) so the UI can surface the real git message.
pub fn git<I, S>(dir: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err.trim().to_string()
        };
        Err(msg)
    }
}

/// Guard values passed as git args so they can't be read as options and can't
/// escape the superproject. Used for branch names and submodule sub-paths.
pub fn safe_arg(s: &str) -> Result<(), String> {
    if s.starts_with('-') {
        return Err(format!("invalid value: {s}"));
    }
    if s.contains("..") || s.starts_with('/') {
        return Err(format!("invalid path: {s}"));
    }
    Ok(())
}

/// Submodule paths of the superproject, parsed from `.gitmodules`. Empty (not
/// an error) when the repo has no submodules.
pub fn submodule_paths(root: &Path) -> Result<Vec<String>, String> {
    // `--get-regexp` prints `submodule.<name>.path <path>` lines.
    let out = match git(
        root,
        ["config", "--file", ".gitmodules", "--get-regexp", "path"],
    ) {
        Ok(o) => o,
        // Missing/blank .gitmodules -> exit 1; treat as "no submodules".
        Err(_) => return Ok(Vec::new()),
    };
    let mut paths: Vec<String> = out
        .lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .map(|(_, p)| p.trim().to_string())
        .filter(|p| !p.is_empty())
        // Reject traversal/option-like paths from a crafted .gitmodules (e.g.
        // `path = ../../other-repo`) so fetch/checkout can never run outside the
        // superproject. Same guard the frontend-supplied `sub` already gets.
        .filter(|p| safe_arg(p).is_ok())
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Local + remote branch names (remotes stripped of `origin/`, HEAD skipped),
/// merged, sorted, deduped.
pub fn branch_list(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(out) = git(dir, ["branch", "--format=%(refname:short)"]) {
        names.extend(out.lines().map(|l| l.trim().to_string()));
    }
    if let Ok(out) = git(dir, ["branch", "-r", "--format=%(refname:short)"]) {
        for l in out.lines() {
            let l = l.trim();
            // Skip symbolic refs like `origin/HEAD -> origin/master`.
            if l.contains("->") {
                continue;
            }
            // Strip the leading remote name (`origin/foo` -> `foo`).
            let name = l.split_once('/').map(|(_, b)| b).unwrap_or(l);
            names.push(name.to_string());
        }
    }
    names.retain(|n| !n.is_empty() && n != "HEAD");
    names.sort();
    names.dedup();
    names
}

/// Ahead/behind vs the tracked upstream. `None` when there's no upstream
/// (detached HEAD, or a branch with no tracking ref).
fn ahead_behind(dir: &Path) -> Option<(u32, u32)> {
    // Order is behind<TAB>ahead: left is @{upstream}, right is HEAD.
    let out = git(
        dir,
        ["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )
    .ok()?;
    let mut it = out.split_whitespace();
    let behind = it.next()?.parse().ok()?;
    let ahead = it.next()?.parse().ok()?;
    Some((ahead, behind))
}

/// Fetch all remotes, then fast-forward the current branch when it tracks an
/// upstream. Returns the first error message, if any. Detached / no-upstream
/// repos are fetched but not pulled.
pub fn fetch_and_pull(dir: &Path) -> Option<String> {
    if let Err(e) = git(dir, ["fetch", "--all", "--prune"]) {
        return Some(e);
    }
    if ahead_behind(dir).is_some() {
        if let Err(e) = git(dir, ["pull", "--ff-only"]) {
            return Some(e);
        }
    }
    None
}

/// Read one repo's row (no network; caller fetches first). `name`/`is_parent`
/// are set by the caller; any git error is captured into `error`.
pub fn read_row(dir: &Path, name: String, is_parent: bool) -> RepoRow {
    let mut row = RepoRow {
        name,
        is_parent,
        ..Default::default()
    };
    match git(dir, ["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(b) if b == "HEAD" => row.detached = true,
        Ok(b) => row.branch = b,
        Err(e) => {
            row.error = Some(e);
            return row;
        }
    }
    row.head_desc = git(dir, ["describe", "--tags", "--always"]).unwrap_or_default();
    row.dirty = git(dir, ["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if let Some((ahead, behind)) = ahead_behind(dir) {
        row.ahead = Some(ahead);
        row.behind = Some(behind);
    }
    row.branches = branch_list(dir);
    row
}

/// `git checkout <branch>` (+ `git pull` when it tracks a remote), applying the
/// requested dirty-tree strategy. `action`: `"stash"` stashes then checks out
/// (stash left in place); `"carry"` checks out keeping uncommitted changes;
/// `"none"` requires a clean tree.
pub fn switch_branch(dir: &Path, branch: &str, action: &str) -> Result<(), String> {
    safe_arg(branch)?;
    let dirty = git(dir, ["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if dirty {
        match action {
            // Stash and leave it: the user can pop it on the new branch.
            "stash" => {
                git(dir, ["stash", "push", "--include-untracked"])?;
            }
            // Carry: plain checkout keeps changes if they don't conflict; git
            // errors out (surfaced) if the switch would overwrite them.
            "carry" => {}
            _ => return Err("working tree has uncommitted changes".into()),
        }
    }
    // DWIM checkout: a remote-only `foo` creates a tracking branch from
    // origin/foo; a local branch just switches.
    git(dir, ["checkout", branch])?;
    // Pull only when the branch has an upstream; a fresh local branch may not.
    if ahead_behind(dir).is_some() {
        git(dir, ["pull", "--ff-only"])?;
    }
    Ok(())
}
