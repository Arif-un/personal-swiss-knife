//! AWS SAML login helper for the `/deploy` page.
//!
//! One button that automates the manual AWS auth dance: open the Google SAML
//! login link in Brave (a chosen profile) in two tabs, wait for the user to
//! manually download the `credentials` file into `~/Downloads/AWS` (a symlink to
//! `~/.aws`), then run the repo's `tools/awsauth` script (which needs Docker for
//! its `docker login` steps) and report whether it logged in.

pub mod commands;

use serde::{Deserialize, Serialize};

/// Google SAML init-SSO link opened (twice) for the AWS login.
pub const LOGIN_URL: &str =
    "https://accounts.google.com/o/saml2/initsso?idpid=C047rowan&spid=864219566680&forceauthn=false";

/// Watched download target: `~/Downloads/AWS` is a symlink to `~/.aws`, so this
/// resolves to `~/.aws/credentials`. A fresh mtime here means the manual download
/// landed and `aws sts` can succeed.
pub const CREDENTIALS_REL: &str = "Downloads/AWS/credentials";

/// Brave `tools/awsauth` lives under the repo root; run it with cwd = repo root
/// so its `tools/devkon/github.token` reads resolve.
pub const AWSAUTH_REL: &str = "tools/awsauth";

fn default_profile() -> String {
    "OP".to_string()
}

fn default_repo_dir() -> String {
    "/Volumes/workspace/netspring".to_string()
}

/// On-disk shape of `awsauth.json`: the two user-editable settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwsAuthConfig {
    /// Brave profile *display* name (what shows in Brave's menu, e.g. `OP`).
    /// Resolved to the on-disk profile directory via Brave's `Local State`.
    #[serde(default = "default_profile")]
    pub brave_profile: String,
    /// Repo root to run `tools/awsauth` from.
    #[serde(default = "default_repo_dir")]
    pub repo_dir: String,
}

impl Default for AwsAuthConfig {
    fn default() -> Self {
        Self {
            brave_profile: default_profile(),
            repo_dir: default_repo_dir(),
        }
    }
}
