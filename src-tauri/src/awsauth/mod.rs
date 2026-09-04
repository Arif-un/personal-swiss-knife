//! AWS SAML login helper for the `/deploy` page.
//!
//! One button that automates the manual AWS auth dance: open the Google SAML
//! login link in Brave (a chosen profile) in two tabs, wait for the user to
//! manually download the `credentials` file into `~/Downloads/AWS` (a symlink to
//! `~/.aws`), then run the repo's `tools/awsauth` script (which needs Docker for
//! its `docker login` steps) and report whether it logged in.

pub mod commands;

use serde::{Deserialize, Serialize};

/// Watched download target: `~/Downloads/AWS` is a symlink to `~/.aws`, so this
/// resolves to `~/.aws/credentials`. A fresh mtime here means the manual download
/// landed and `aws sts` can succeed.
pub const CREDENTIALS_REL: &str = "Downloads/AWS/credentials";

/// `tools/awsauth` lives under the repo root; run it with cwd = repo root so its
/// relative reads resolve.
pub const AWSAUTH_REL: &str = "tools/awsauth";

/// On-disk shape of `awsauth.json`: the user-editable settings. All blank by
/// default — set them in Settings (kept out of the repo so nothing org-specific
/// ships).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsAuthConfig {
    /// Brave profile *display* name (what shows in Brave's menu, e.g. `OP`).
    /// Resolved to the on-disk profile directory via Brave's `Local State`.
    #[serde(default)]
    pub brave_profile: String,
    /// Repo root to run `tools/awsauth` from.
    #[serde(default)]
    pub repo_dir: String,
    /// SAML/SSO login URL opened (twice) in Brave for the AWS login.
    #[serde(default)]
    pub login_url: String,
}
