//! App-wide settings shared across features.
//!
//! Two concerns live here:
//! - **Branding** (`branding.json`): the display name + accent colour the UI
//!   reads at runtime, so the app can be rebranded without a rebuild.
//! - **Backup / restore**: export every persisted config in the app data dir
//!   (all `*.json`, branding included) plus the SSH keychain secrets into one
//!   file, and import it back. Lets a user move their whole setup between
//!   machines.

pub mod commands;

use serde::{Deserialize, Serialize};

/// Default display name shown in the header, window title and home page.
fn default_display_name() -> String {
    "Swiss Knife".to_string()
}

/// Default accent, applied to `--primary` / `--sidebar-primary` / `--ring`.
fn default_accent() -> String {
    "oklch(0.488 0.243 264.376)".to_string()
}

/// On-disk shape of `branding.json` — the user-facing look-and-feel knobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branding {
    #[serde(default = "default_display_name")]
    pub display_name: String,
    /// Any CSS colour string (`oklch(...)`, `#rrggbb`, `hsl(...)`, ...). Applied
    /// verbatim to the accent CSS variables, so no colour-space conversion here.
    #[serde(default = "default_accent")]
    pub accent_color: String,
}

impl Default for Branding {
    fn default() -> Self {
        Self {
            display_name: default_display_name(),
            accent_color: default_accent(),
        }
    }
}
