//! Git submodule dashboard page (`/submodules`).
//!
//! Points at one configurable superproject dir (a git repo with `.gitmodules`)
//! and lists the parent repo plus every submodule as a row: current branch,
//! dirty/clean, ahead/behind vs upstream, and the local+remote branch list for
//! a per-row switch. Switching runs `git checkout` (+ `git pull`); a dirty tree
//! is either stashed or carried along per the caller's choice. Switching the
//! parent's branch leaves submodules as-is (no `git submodule update`).
//!
//! All git work shells out to the `git` CLI (no libgit2 dep), mirroring how the
//! devkon feature shells out to `gh`.

pub mod commands;
pub mod git;

use serde::{Deserialize, Serialize};

/// Persisted config: the single superproject path we operate on.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitmodConfig {
    #[serde(default)]
    pub path: String,
}

/// One repo row: the parent superproject or a submodule.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepoRow {
    /// Display name: `.` for the parent, else the submodule path.
    pub name: String,
    pub is_parent: bool,
    /// Current branch, or empty when detached.
    pub branch: String,
    pub detached: bool,
    /// `git describe --tags --always` of HEAD (tag or short sha).
    pub head_desc: String,
    pub dirty: bool,
    /// Commits ahead/behind the tracked upstream. `None` when no upstream
    /// (e.g. detached at a tag).
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Local + remote branch names, merged and sorted, for the switch picker.
    pub branches: Vec<String>,
    /// Per-repo error (e.g. fetch failed). Row still renders with local info.
    pub error: Option<String>,
}

/// Result of a bulk "switch all" run: the refreshed rows plus human-readable
/// notes for repos that were skipped (no target branch) or failed to switch.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwitchAllResult {
    pub rows: Vec<RepoRow>,
    pub notes: Vec<String>,
}
