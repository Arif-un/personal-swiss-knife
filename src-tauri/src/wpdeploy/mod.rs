//! Build & deploy WordPress plugins from the `envira-dev` monorepo to a live
//! site (`/submodules` page, per-row deploy action).
//!
//! This is a native Rust port of the standalone `envira-devsite-deploy` CLI
//! tool. The volatile part (building the shippable zip) is NOT reimplemented: it
//! shells out to `envira-dev`'s own `yarn actions zip <slug> <dir>`, the single
//! source of truth. Only the thin orchestration (build trigger, upload, backup,
//! `wp plugin install --force`, activate, cache flush, rollback) lives here.
//!
//! Remote work uses the system `ssh`/`scp` binaries (matching the proven CLI
//! tool) rather than reimplementing sftp over russh. Hosts are resolved from the
//! same store the SSH page uses (`~/.ssh/config` + app `hosts.json`); the only
//! deploy-specific extra is a per-host docroot, kept in `wpdeploy.json`.

pub mod commands;
pub mod products;

use serde::{Deserialize, Serialize};

/// Tauri event: one line of deploy output (build/upload/install/...).
pub const EVENT_LOG: &str = "wpdeploy://log";
/// Tauri event: a deploy/rollback finished (success or failure).
pub const EVENT_DONE: &str = "wpdeploy://done";

/// Maps a submodule repo folder to the product group it ships. Replaces the old
/// hardcoded, org-specific match so the whole product map is user-editable and
/// nothing product-specific ships in the repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoMapping {
    /// Submodule folder name (e.g. `envira-gallery-lite`).
    pub repo: String,
    /// Product group key — must match a key in the monorepo's product-slugs JSON
    /// (ignored for `theme`).
    pub group: String,
    /// `lite` | `pro` | `theme`.
    pub kind: String,
}

/// Persisted deploy settings (`wpdeploy.json` in the app data dir).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WpDeployConfig {
    /// Id of the globally selected target host (from the SSH host store).
    #[serde(default)]
    pub target_host_id: String,
    /// Local base dir for built zips (`<base>/<group>/<slug>.zip`).
    #[serde(default)]
    pub zip_base: String,
    /// WordPress docroot per host id (host store has no docroot field).
    #[serde(default)]
    pub docroots: std::collections::HashMap<String, String>,
    /// Theme product slug (the theme is not in the product-slugs JSON). Empty =
    /// no theme product.
    #[serde(default)]
    pub theme_slug: String,
    /// Path to the product-slugs JSON, relative to the monorepo root. Empty = no
    /// plugin products resolvable.
    #[serde(default)]
    pub slugs_rel_path: String,
    /// Repo-folder -> product-group map. Empty = nothing deployable until set.
    #[serde(default)]
    pub repo_map: Vec<RepoMapping>,
}

/// One deployable product inside a repo (a plugin slug, or the theme).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    /// `envira` | `soliloquy` | `nextgen` | `cdn` | `theme`.
    pub group: String,
    /// The `yarn actions zip` target slug.
    pub slug: String,
    pub is_lite: bool,
    /// Group has a separate local asset build (UI offers a build-first toggle).
    pub buildable: bool,
}

/// One streamed output line for a running deploy, tagged with its deploy id so
/// the UI can route it to the right log panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub deploy_id: String,
    /// `"step"` (a stage banner), `"out"` (stdout) or `"err"` (stderr).
    pub stream: String,
    pub line: String,
}

/// Terminal event for a deploy/rollback.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoneEvent {
    pub deploy_id: String,
    pub ok: bool,
    pub message: String,
    /// Installed plugin version on success (best-effort).
    pub version: Option<String>,
}
