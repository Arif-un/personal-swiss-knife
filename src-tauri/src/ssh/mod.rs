use serde::{Deserialize, Serialize};

pub mod commands;
pub mod config;
pub mod forward;
pub mod keychain;
pub mod known_hosts;
pub mod session;

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
    /// "ssh-config" or "app".
    #[serde(default = "default_source")]
    pub source: String,
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

fn default_forward_kind() -> String {
    "L".into()
}
fn default_bind_addr() -> String {
    "127.0.0.1".into()
}
fn default_source() -> String {
    "app".into()
}
fn default_port() -> u16 {
    22
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
impl From<String> for SshError {
    fn from(s: String) -> Self {
        SshError::Msg(s)
    }
}
impl From<&str> for SshError {
    fn from(s: &str) -> Self {
        SshError::Msg(s.to_string())
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
