//! Dev-cluster deploy/destroy page (`/deploy`).
//!
//! A list of user-managed names; each maps to an isolated namespace whose URL is
//! `cluster_domain` with `{name}` substituted. Deploy/destroy dispatch a GitHub
//! Actions `workflow_dispatch` workflow (repo + workflow file configured in
//! Settings) via `gh`, reusing the same CLI layer as the GitHub PR feature. `gh
//! workflow run` doesn't return the dispatched run id, so after dispatch we watch
//! `gh run list` for the ref and capture the newest run whose id differs from the
//! pre-dispatch one, storing it per name for status lookups.

pub mod commands;

use serde::{Deserialize, Serialize};

fn default_mode() -> String {
    "full".to_string()
}

/// One managed name = one devkon namespace deployment.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DevkonEntry {
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Git ref deployed for this name.
    #[serde(default)]
    pub branch: String,
    /// Deploy mode: `full` | `backend` | `cleanFull` | `cleanBackend`.
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Run id of the last deploy/destroy this app dispatched for this name.
    #[serde(default)]
    pub last_run_id: Option<u64>,
    /// `"apply"` or `"destroy"` - what the last tracked run was doing.
    #[serde(default)]
    pub last_run_kind: Option<String>,
    /// Newest run id on the branch at the moment of the last dispatch. Only used
    /// while a dispatch is "awaiting" (kind set but no run id captured within the
    /// watch window): a later status poll adopts the first run whose id differs.
    #[serde(default)]
    pub baseline_run_id: Option<u64>,
    /// Web URL of the last tracked run.
    #[serde(default)]
    pub last_run_url: Option<String>,
    /// Completion time (ISO8601) of the last successful apply, per GitHub.
    #[serde(default)]
    pub last_deployed_at: Option<String>,
}

/// On-disk shape of `devkon.json`. Repo/workflow/domain are blank by default and
/// set in Settings, so no org-specific target ships in the repo.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DevkonStore {
    /// `owner/repo` hosting the deploy workflow.
    #[serde(default)]
    pub repo: String,
    /// The `workflow_dispatch` workflow file name.
    #[serde(default)]
    pub workflow: String,
    /// Namespace URL template; `{name}` is replaced with the entry name. Empty =
    /// no clickable URL shown.
    #[serde(default)]
    pub cluster_domain: String,
    #[serde(default)]
    pub entries: Vec<DevkonEntry>,
}

/// Live status of a name's last tracked run, returned to the UI.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunStatus {
    pub run_id: Option<u64>,
    /// `"apply"` or `"destroy"`.
    pub kind: Option<String>,
    /// `queued` | `in_progress` | `completed` | `none` (no run tracked yet).
    pub state: String,
    /// `success` | `failure` | ... (only when `state == "completed"`).
    pub conclusion: Option<String>,
    pub last_deployed_at: Option<String>,
}

/// Map a deploy mode to the workflow's `type` input and `clean` flag.
pub fn mode_params(mode: &str) -> (&'static str, bool) {
    match mode {
        "backend" => ("backend", false),
        "cleanFull" => ("full", true),
        "cleanBackend" => ("backend", true),
        _ => ("full", false),
    }
}

#[cfg(test)]
mod tests {
    use super::mode_params;

    #[test]
    fn mode_params_maps_type_and_clean() {
        assert_eq!(mode_params("full"), ("full", false));
        assert_eq!(mode_params("backend"), ("backend", false));
        assert_eq!(mode_params("cleanFull"), ("full", true));
        assert_eq!(mode_params("cleanBackend"), ("backend", true));
        // Unknown / empty falls back to a safe full, no clean.
        assert_eq!(mode_params("garbage"), ("full", false));
    }
}
