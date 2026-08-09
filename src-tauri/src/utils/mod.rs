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

use serde::Serialize;

/// Umbrella profile. Present = module active; absent (moved aside) = disabled.
const ORGINFO: &str = "/opt/cisco/secureclient/umbrella/OrgInfo.json";
const ORGINFO_OFF: &str = "/opt/cisco/secureclient/umbrella/OrgInfo.json.disabled";
/// launchd daemon running vpnagentd, which spawns and keeps alive acumbrellaagent.
const DAEMON_LABEL: &str = "com.cisco.secureclient.vpn.service.agent";
const DAEMON_PLIST: &str = "/opt/cisco/secureclient/bin/Cisco Secure Client - AnyConnect VPN Service.app/Contents/Library/LaunchDaemons/com.cisco.secureclient.vpn.service.agent.plist";

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
