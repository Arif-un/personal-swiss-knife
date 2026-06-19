use std::path::PathBuf;

use crate::ssh::{ForwardSpec, Host, SshError, SshResult};

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

fn parse(path: &PathBuf) -> Parsed {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut blocks: Vec<Block> = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("host ") || lower == "host" {
            let alias = trimmed[4..].trim().split_whitespace().next().unwrap_or("").to_string();
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
                blocks.push(Block { alias, start, end, host });
            }
        } else {
            i += 1;
        }
    }

    Parsed { lines, blocks }
}

fn block_to_host(alias: &str, body: &[String]) -> Host {
    let mut host = Host {
        id: format!("cfg:{alias}"),
        source: "ssh-config".into(),
        alias: alias.to_string(),
        hostname: String::new(),
        user: String::new(),
        port: 22,
        identity_file: None,
        use_agent: true,
        proxy_jump: None,
        forwards: Vec::new(),
        extra_options: None,
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
            "port" => host.port = value.parse().unwrap_or(22),
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
        None => ("127.0.0.1".to_string(), local.parse().ok()?),
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

/// All hosts defined in `~/.ssh/config`.
pub fn parse_ssh_config() -> Vec<Host> {
    let path = match ssh_config_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    parse(&path).blocks.into_iter().map(|b| b.host).collect()
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
    if host.port != 22 {
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

fn backup_and_write(path: &PathBuf, lines: &[String]) -> SshResult<()> {
    if path.exists() {
        let backup = path.with_extension("bak-swissknife");
        let _ = std::fs::copy(path, &backup);
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
    let mut parsed = parse(&path);
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
    let mut parsed = parse(&path);
    if let Some(block) = parsed.blocks.iter().find(|b| b.alias == alias) {
        let (start, end) = (block.start, block.end);
        parsed.lines.splice(start..end, std::iter::empty());
        backup_and_write(&path, &parsed.lines)?;
    }
    Ok(())
}

// ---- app-owned host store (JSON) ----

pub fn load_app_hosts(store_path: &PathBuf) -> SshResult<Vec<Host>> {
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

pub fn save_app_hosts(store_path: &PathBuf, hosts: &[Host]) -> SshResult<()> {
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(hosts)?;
    std::fs::write(store_path, data)?;
    Ok(())
}

pub fn upsert_app_host(store_path: &PathBuf, mut host: Host) -> SshResult<String> {
    let mut hosts = load_app_hosts(store_path)?;
    host.source = "app".into();
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

pub fn delete_app_host(store_path: &PathBuf, id: &str) -> SshResult<()> {
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
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(22)),
        None => (rest.to_string(), 22),
    };
    Host {
        id: format!("jump:{token}"),
        source: "ssh-config".into(),
        alias: hostname.clone(),
        hostname,
        user,
        port,
        identity_file: None,
        use_agent: true,
        proxy_jump: None,
        forwards: Vec::new(),
        extra_options: None,
    }
}

impl SshError {
    pub fn msg(s: impl Into<String>) -> Self {
        SshError::Msg(s.into())
    }
}
