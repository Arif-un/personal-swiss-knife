//! Small system utilities exposed on the `/utils` page.
//!
//! Currently: toggling Cisco Umbrella (the `acumbrellaagent` process, shipped as
//! part of Cisco Secure Client). Umbrella has no launchd job of its own - the
//! `vpnagentd` daemon (KeepAlive=true) respawns it - so the only reversible,
//! respawn-proof control point is the Umbrella profile (`OrgInfo.json`): move it
//! aside and restart the daemon and only Umbrella goes dark, leaving the VPN side
//! intact. Enable/disable needs root, so it runs through an
//! `osascript ... with administrator privileges` prompt (native macOS auth).

pub mod commands;

use serde::{Deserialize, Serialize};

/// Umbrella profile. Present = module active; absent (moved aside) = disabled.
fn default_orginfo() -> String {
    "/opt/cisco/secureclient/umbrella/OrgInfo.json".to_string()
}
fn default_orginfo_off() -> String {
    "/opt/cisco/secureclient/umbrella/OrgInfo.json.disabled".to_string()
}
/// launchd daemon running vpnagentd, which spawns and keeps alive acumbrellaagent.
fn default_daemon_label() -> String {
    "com.cisco.secureclient.vpn.service.agent".to_string()
}
fn default_daemon_plist() -> String {
    "/opt/cisco/secureclient/bin/Cisco Secure Client - AnyConnect VPN Service.app/Contents/Library/LaunchDaemons/com.cisco.secureclient.vpn.service.agent.plist".to_string()
}

/// On-disk shape of `cisco.json`. Defaults are the standard macOS Cisco Secure
/// Client install paths (not org-specific); editable in Settings for non-default
/// installs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiscoConfig {
    #[serde(default = "default_orginfo")]
    pub orginfo: String,
    #[serde(default = "default_orginfo_off")]
    pub orginfo_off: String,
    #[serde(default = "default_daemon_label")]
    pub daemon_label: String,
    #[serde(default = "default_daemon_plist")]
    pub daemon_plist: String,
}

impl Default for CiscoConfig {
    fn default() -> Self {
        Self {
            orginfo: default_orginfo(),
            orginfo_off: default_orginfo_off(),
            daemon_label: default_daemon_label(),
            daemon_plist: default_daemon_plist(),
        }
    }
}

/// Snapshot of Cisco Umbrella state for the `/utils` page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiscoStatus {
    /// Cisco Secure Client is installed (the daemon plist exists).
    pub installed: bool,
    /// The Umbrella agent (`acumbrellaagent`) is currently running.
    pub running: bool,
    /// The Umbrella profile is in place, i.e. the module is enabled.
    pub profile_present: bool,
}
