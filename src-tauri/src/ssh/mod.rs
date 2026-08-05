use serde::{Deserialize, Serialize};

pub mod commands;
pub mod config;
pub mod discover;
pub mod forward;
pub mod keychain;
pub mod known_hosts;
pub mod session;

/// Default SSH port when a host does not specify one.
pub const DEFAULT_SSH_PORT: u16 = 22;
/// Default local bind address for port forwards.
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1";

/// Tauri event names emitted by the SSH layer.
pub const EVENT_SSH_DATA: &str = "ssh://data";
pub const EVENT_SSH_CLOSED: &str = "ssh://closed";
pub const EVENT_SSH_HOSTKEY: &str = "ssh://hostkey";

/// Where a host definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HostSource {
    /// Owned by the app's `hosts.json` store.
    #[default]
    App,
    /// Parsed from (and written back to) `~/.ssh/config`.
    SshConfig,
}

/// A `-L` local port-forward specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardSpec {
    /// Forward type — only "L" (local) is supported in v1.
    #[serde(default = "default_forward_kind", rename = "type")]
    pub kind: String,
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    pub bind_port: u16,
    pub dest_host: String,
    pub dest_port: u16,
}

/// A saved SSH host, either parsed from `~/.ssh/config` or owned by the app store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: HostSource,
    pub alias: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub user: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default = "default_true")]
    pub use_agent: bool,
    #[serde(default)]
    pub proxy_jump: Option<String>,
    #[serde(default)]
    pub forwards: Vec<ForwardSpec>,
    #[serde(default)]
    pub extra_options: Option<String>,
}

impl Default for Host {
    fn default() -> Self {
        Host {
            id: String::new(),
            source: HostSource::default(),
            alias: String::new(),
            hostname: String::new(),
            user: String::new(),
            port: DEFAULT_SSH_PORT,
            identity_file: None,
            use_agent: true,
            proxy_jump: None,
            forwards: Vec::new(),
            extra_options: None,
        }
    }
}

fn default_forward_kind() -> String {
    "L".into()
}
fn default_bind_addr() -> String {
    DEFAULT_BIND_ADDR.into()
}
fn default_port() -> u16 {
    DEFAULT_SSH_PORT
}
fn default_true() -> bool {
    true
}

/// Unified error type for the SSH layer. Converted to a `String` at the Tauri
/// command boundary (Tauri command errors must be `Serialize`).
#[derive(Debug, thiserror::Error)]
pub enum SshError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Russh(#[from] russh::Error),
    #[error(transparent)]
    Keys(#[from] russh::keys::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl From<SshError> for String {
    fn from(e: SshError) -> Self {
        e.to_string()
    }
}
impl From<&str> for SshError {
    fn from(s: &str) -> Self {
        SshError::Msg(s.to_string())
    }
}

impl SshError {
    pub fn msg(s: impl Into<String>) -> Self {
        SshError::Msg(s.into())
    }
}

pub type SshResult<T> = std::result::Result<T, SshError>;

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// Best-effort current username for hosts that omit `User`.
pub fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "root".into())
}
