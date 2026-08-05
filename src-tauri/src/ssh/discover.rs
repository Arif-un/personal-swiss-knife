use std::collections::HashSet;
use std::path::PathBuf;

use crate::ssh::{Host, DEFAULT_SSH_PORT};

fn history_files() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = dirs::home_dir() {
        v.push(home.join(".zsh_history"));
        v.push(home.join(".bash_history"));
        v.push(home.join(".local/share/fish/fish_history"));
    }
    v
}

/// Strip the per-shell line prefix (zsh `: <ts>:<dur>;`, fish `- cmd: `).
fn strip_prefix(line: &str) -> &str {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix(": ") {
        if let Some((_, cmd)) = rest.split_once(';') {
            return cmd;
        }
    }
    if let Some(rest) = t.strip_prefix("- cmd:") {
        return rest.trim_start();
    }
    t
}

/// Parse a single shell command segment into a Host if it is an `ssh` invocation
/// with an explicit target (`user@host` or a FQDN/IP).
fn parse_ssh(cmd: &str) -> Option<Host> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let pos = tokens.iter().position(|t| *t == "ssh")?;

    let mut i = pos + 1;
    let mut user = String::new();
    let mut port: u16 = DEFAULT_SSH_PORT;
    let mut identity: Option<String> = None;
    let mut proxy: Option<String> = None;
    let mut target: Option<String> = None;

    while i < tokens.len() {
        let t = tokens[i];
        match t {
            "-i" => {
                i += 1;
                identity = tokens.get(i).map(|s| s.trim_matches(['"', '\'']).to_string());
            }
            "-p" => {
                i += 1;
                if let Some(p) = tokens.get(i) {
                    port = p.parse().unwrap_or(DEFAULT_SSH_PORT);
                }
            }
            "-l" => {
                i += 1;
                user = tokens.get(i).map(|s| s.to_string()).unwrap_or_default();
            }
            "-J" => {
                i += 1;
                proxy = tokens.get(i).map(|s| s.to_string());
            }
            // flags that consume a value we don't keep
            "-L" | "-R" | "-D" | "-o" | "-b" | "-c" | "-m" | "-F" | "-E" | "-W" | "-w" => {
                i += 1;
            }
            _ if t.starts_with('-') => { /* valueless flag */ }
            _ => {
                target = Some(t.trim_matches(['"', '\'']).to_string());
                break;
            }
        }
        i += 1;
    }

    let target = target?;
    let has_at = target.contains('@');
    let (parsed_user, host) = match target.split_once('@') {
        Some((u, h)) => (u.to_string(), h.to_string()),
        None => (String::new(), target.clone()),
    };
    if !parsed_user.is_empty() {
        user = parsed_user;
    }

    // Only accept targets we can actually connect to: explicit user@host, or a
    // dotted hostname / IP. Bare words are likely config aliases or noise.
    if !has_at && !host.contains('.') {
        return None;
    }
    if host.is_empty() {
        return None;
    }

    let alias = host.split('.').next().unwrap_or(&host).to_string();
    Some(Host {
        alias,
        hostname: host,
        user,
        port,
        identity_file: identity,
        proxy_jump: proxy,
        ..Host::default()
    })
}

/// Scan shell history files for past `ssh` targets, de-duplicated by user/host/port.
pub fn discover_hosts() -> Vec<Host> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for file in history_files() {
        let bytes = match std::fs::read(&file) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let content = String::from_utf8_lossy(&bytes);
        for raw in content.lines() {
            let line = strip_prefix(raw);
            for segment in line.split(['|', ';', '&']) {
                if let Some(host) = parse_ssh(segment) {
                    let key = format!("{}@{}:{}", host.user, host.hostname, host.port);
                    if seen.insert(key) {
                        out.push(host);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_user_at_host() {
        let host = parse_ssh("ssh deploy@example.com").unwrap();
        assert_eq!(host.user, "deploy");
        assert_eq!(host.hostname, "example.com");
        assert_eq!(host.alias, "example");
    }

    #[test]
    fn parse_ssh_with_port_and_identity() {
        let host = parse_ssh("ssh -p 2222 -i ~/.ssh/id_ed25519 deploy@10.0.0.1").unwrap();
        assert_eq!(host.port, 2222);
        assert_eq!(host.identity_file.as_deref(), Some("~/.ssh/id_ed25519"));
        assert_eq!(host.hostname, "10.0.0.1");
    }

    #[test]
    fn parse_ssh_rejects_bare_alias() {
        // No `@` and no dot: likely a config alias, not a connectable target.
        assert!(parse_ssh("ssh myalias").is_none());
    }

    #[test]
    fn strip_prefix_zsh_and_fish() {
        assert_eq!(strip_prefix(": 1700000000:0;ssh host"), "ssh host");
        assert_eq!(strip_prefix("- cmd: ssh host"), "ssh host");
    }
}
