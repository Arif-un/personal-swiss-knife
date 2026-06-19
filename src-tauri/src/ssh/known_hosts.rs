use russh::keys::PublicKey;

/// Outcome of checking a server key against `~/.ssh/known_hosts`.
pub enum HostKeyStatus {
    /// Host is known and the key matches — connect silently.
    Trusted,
    /// Host is not in known_hosts — caller must prompt the user.
    Unknown,
    /// Host is known but the key changed — refuse (possible MITM).
    Changed,
}

pub fn check(host: &str, port: u16, key: &PublicKey) -> HostKeyStatus {
    match russh::keys::check_known_hosts(host, port, key) {
        Ok(true) => HostKeyStatus::Trusted,
        Ok(false) => HostKeyStatus::Unknown,
        Err(_) => HostKeyStatus::Changed,
    }
}

pub fn learn(host: &str, port: u16, key: &PublicKey) {
    let _ = russh::keys::known_hosts::learn_known_hosts(host, port, key);
}
