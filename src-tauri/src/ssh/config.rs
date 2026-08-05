use std::path::{Path, PathBuf};

use crate::ssh::{ForwardSpec, Host, HostSource, SshResult, DEFAULT_BIND_ADDR, DEFAULT_SSH_PORT};

/// Path to `~/.ssh/config`.
pub fn ssh_config_path() -> SshResult<PathBuf> {
    let home = dirs::home_dir().ok_or("cannot locate home directory")?;
    Ok(home.join(".ssh").join("config"))
}

/// A parsed `Host` block, tracking its line range for safe in-place rewrite.
struct Block {
    alias: String,
    start: usize,
    end: usize, // exclusive
    host: Host,
}

struct Parsed {
    lines: Vec<String>,
    blocks: Vec<Block>,
}

/// Read a config file into lines. A missing file is an empty document; any other
/// IO error is surfaced so callers never overwrite a file they failed to read.
fn read_config_lines(path: &Path) -> SshResult<Vec<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content.lines().map(|l| l.to_string()).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn parse_lines(lines: Vec<String>) -> Parsed {
    let mut blocks: Vec<Block> = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("host ") || lower == "host" {
            let alias = trimmed[4..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            let start = i;
            i += 1;
            while i < lines.len() {
                let t = lines[i].trim().to_lowercase();
                if t.starts_with("host ") || t == "host" {
                    break;
                }
                i += 1;
            }
            let end = i;
            if !alias.is_empty() {
                let host = block_to_host(&alias, &lines[start..end]);
                blocks.push(Block {
                    alias,
                    start,
                    end,
                    host,
                });
            }
        } else {
            i += 1;
        }
    }

    Parsed { lines, blocks }
}

fn parse_file(path: &Path) -> SshResult<Parsed> {
    Ok(parse_lines(read_config_lines(path)?))
}

fn block_to_host(alias: &str, body: &[String]) -> Host {
    let mut host = Host {
        id: format!("cfg:{alias}"),
        source: HostSource::SshConfig,
        alias: alias.to_string(),
        ..Host::default()
    };
    let mut extra: Vec<String> = Vec::new();

    for line in body.iter().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (key, value) = match trimmed.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (k.trim(), v.trim_start_matches('=').trim()),
            None => continue,
        };
        match key.to_lowercase().as_str() {
            "hostname" => host.hostname = value.to_string(),
            "user" => host.user = value.to_string(),
            "port" => host.port = value.parse().unwrap_or(DEFAULT_SSH_PORT),
            "identityfile" => host.identity_file = Some(value.to_string()),
            "proxyjump" => host.proxy_jump = Some(value.to_string()),
            "localforward" => {
                if let Some(spec) = parse_local_forward(value) {
                    host.forwards.push(spec);
                }
            }
            _ => extra.push(format!("{key} {value}")),
        }
    }
    if host.hostname.is_empty() {
        host.hostname = alias.to_string();
    }
    if !extra.is_empty() {
        host.extra_options = Some(extra.join("\n"));
    }
    host
}

/// Parse `LocalForward [bind:]port host:hostport`.
fn parse_local_forward(value: &str) -> Option<ForwardSpec> {
    let mut parts = value.split_whitespace();
    let local = parts.next()?;
    let remote = parts.next()?;

    let (bind_addr, bind_port) = match local.rsplit_once(':') {
        Some((addr, port)) => (addr.to_string(), port.parse().ok()?),
        None => (DEFAULT_BIND_ADDR.to_string(), local.parse().ok()?),
    };
    let (dest_host, dest_port) = remote.rsplit_once(':')?;
    Some(ForwardSpec {
        kind: "L".into(),
        bind_addr,
        bind_port,
        dest_host: dest_host.to_string(),
        dest_port: dest_port.parse().ok()?,
    })
}

/// All hosts defined in `~/.ssh/config`. Read failures yield an empty list so a
/// broken config never blocks listing app-owned hosts.
pub fn parse_ssh_config() -> Vec<Host> {
    let path = match ssh_config_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    match parse_file(&path) {
        Ok(parsed) => parsed.blocks.into_iter().map(|b| b.host).collect(),
        Err(_) => Vec::new(),
    }
}

/// Render a host as an OpenSSH `Host` block (indented body).
fn render_block(host: &Host) -> Vec<String> {
    let mut out = vec![format!("Host {}", host.alias)];
    if !host.hostname.is_empty() && host.hostname != host.alias {
        out.push(format!("    HostName {}", host.hostname));
    }
    if !host.user.is_empty() {
        out.push(format!("    User {}", host.user));
    }
    if host.port != DEFAULT_SSH_PORT {
        out.push(format!("    Port {}", host.port));
    }
    if let Some(id) = &host.identity_file {
        if !id.is_empty() {
            out.push(format!("    IdentityFile {id}"));
        }
    }
    if let Some(pj) = &host.proxy_jump {
        if !pj.is_empty() {
            out.push(format!("    ProxyJump {pj}"));
        }
    }
    for f in &host.forwards {
        out.push(format!(
            "    LocalForward {}:{} {}:{}",
            f.bind_addr, f.bind_port, f.dest_host, f.dest_port
        ));
    }
    if let Some(extra) = &host.extra_options {
        for line in extra.lines() {
            let line = line.trim();
            if !line.is_empty() {
                out.push(format!("    {line}"));
            }
        }
    }
    out
}

fn backup_and_write(path: &Path, lines: &[String]) -> SshResult<()> {
    if path.exists() {
        // Fail rather than overwrite a file we could not back up.
        let backup = path.with_extension("bak-swissknife");
        std::fs::copy(path, &backup)?;
    } else if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut content = lines.join("\n");
    content.push('\n');
    std::fs::write(path, content)?;
    Ok(())
}

/// Write a host back into `~/.ssh/config`: replace its existing block in place
/// (preserving every other byte), or append a new managed block.
pub fn write_host_block(host: &Host) -> SshResult<()> {
    let path = ssh_config_path()?;
    let mut parsed = parse_file(&path)?;
    let rendered = render_block(host);

    if let Some(block) = parsed.blocks.iter().find(|b| b.alias == host.alias) {
        let (start, end) = (block.start, block.end);
        parsed.lines.splice(start..end, rendered);
    } else {
        if !parsed.lines.is_empty() && !parsed.lines.last().map(|l| l.is_empty()).unwrap_or(true) {
            parsed.lines.push(String::new());
        }
        parsed.lines.extend(rendered);
    }
    backup_and_write(&path, &parsed.lines)
}

/// Remove a host's block from `~/.ssh/config`.
pub fn delete_host_block(alias: &str) -> SshResult<()> {
    let path = ssh_config_path()?;
    let mut parsed = parse_file(&path)?;
    if let Some(block) = parsed.blocks.iter().find(|b| b.alias == alias) {
        let (start, end) = (block.start, block.end);
        parsed.lines.splice(start..end, std::iter::empty());
        backup_and_write(&path, &parsed.lines)?;
    }
    Ok(())
}

// ---- app-owned host store (JSON) ----

pub fn load_app_hosts(store_path: &Path) -> SshResult<Vec<Host>> {
    if !store_path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(store_path)?;
    if data.trim().is_empty() {
        return Ok(Vec::new());
    }
    let hosts: Vec<Host> = serde_json::from_str(&data)?;
    Ok(hosts)
}

pub fn save_app_hosts(store_path: &Path, hosts: &[Host]) -> SshResult<()> {
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(hosts)?;
    std::fs::write(store_path, data)?;
    Ok(())
}

pub fn upsert_app_host(store_path: &Path, mut host: Host) -> SshResult<String> {
    let mut hosts = load_app_hosts(store_path)?;
    host.source = HostSource::App;
    if host.id.is_empty() {
        host.id = format!("app:{}", uuid::Uuid::new_v4());
    }
    let id = host.id.clone();
    match hosts.iter_mut().find(|h| h.id == host.id) {
        Some(existing) => *existing = host,
        None => hosts.push(host),
    }
    save_app_hosts(store_path, &hosts)?;
    Ok(id)
}

pub fn delete_app_host(store_path: &Path, id: &str) -> SshResult<()> {
    let mut hosts = load_app_hosts(store_path)?;
    hosts.retain(|h| h.id != id);
    save_app_hosts(store_path, &hosts)
}

/// Resolve a ProxyJump token (`alias`, `user@host`, `user@host:port`) to a Host,
/// preferring a matching saved host by alias.
pub fn resolve_jump(token: &str, all: &[Host]) -> Host {
    let token = token.trim();
    if let Some(h) = all.iter().find(|h| h.alias == token) {
        return h.clone();
    }
    let (user, rest) = match token.split_once('@') {
        Some((u, r)) => (u.to_string(), r),
        None => (String::new(), token),
    };
    let (hostname, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(DEFAULT_SSH_PORT)),
        None => (rest.to_string(), DEFAULT_SSH_PORT),
    };
    Host {
        id: format!("jump:{token}"),
        source: HostSource::SshConfig,
        alias: hostname.clone(),
        hostname,
        user,
        port,
        ..Host::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_forward_with_bind_addr() {
        let spec = parse_local_forward("0.0.0.0:8080 db:5432").unwrap();
        assert_eq!(spec.bind_addr, "0.0.0.0");
        assert_eq!(spec.bind_port, 8080);
        assert_eq!(spec.dest_host, "db");
        assert_eq!(spec.dest_port, 5432);
    }

    #[test]
    fn parse_local_forward_defaults_bind_addr() {
        let spec = parse_local_forward("8080 db:5432").unwrap();
        assert_eq!(spec.bind_addr, DEFAULT_BIND_ADDR);
        assert_eq!(spec.bind_port, 8080);
    }

    #[test]
    fn parse_local_forward_rejects_garbage() {
        assert!(parse_local_forward("nope").is_none());
    }

    #[test]
    fn block_to_host_parses_directives() {
        let body = vec![
            "Host web".to_string(),
            "    HostName example.com".to_string(),
            "    User deploy".to_string(),
            "    Port 2222".to_string(),
        ];
        let host = block_to_host("web", &body);
        assert_eq!(host.hostname, "example.com");
        assert_eq!(host.user, "deploy");
        assert_eq!(host.port, 2222);
        assert_eq!(host.source, HostSource::SshConfig);
    }
}
