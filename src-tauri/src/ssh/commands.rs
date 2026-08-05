use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::ssh::session::{SessionCmd, SshState};
use crate::ssh::{config, keychain, session, ForwardSpec, Host, HostSource, DEFAULT_SSH_PORT};

const HOSTS_FILE: &str = "hosts.json";

fn app_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(HOSTS_FILE))
}

fn all_hosts(app: &AppHandle) -> Result<Vec<Host>, String> {
    let mut hosts = config::parse_ssh_config();
    let store = app_store_path(app)?;
    hosts.extend(config::load_app_hosts(&store)?);
    Ok(hosts)
}

/// Reject control characters / newlines in single-line host fields so a saved
/// host can never inject extra directives into `~/.ssh/config` (e.g. a newline
/// followed by `ProxyCommand`). `extra_options` is intentionally multi-line and
/// is validated only for an empty alias / carriage returns.
fn validate_host(host: &Host) -> Result<(), String> {
    fn ok_single_line(value: &str) -> bool {
        !value.chars().any(|c| c.is_control())
    }

    if host.alias.trim().is_empty() {
        return Err("host alias is required".into());
    }
    if host.alias.split_whitespace().count() != 1 {
        return Err("host alias must not contain whitespace".into());
    }

    let single_line: [(&str, &str); 4] = [
        ("alias", &host.alias),
        ("hostname", &host.hostname),
        ("user", &host.user),
        ("identity file", host.identity_file.as_deref().unwrap_or("")),
    ];
    for (label, value) in single_line {
        if !ok_single_line(value) {
            return Err(format!("{label} must not contain control characters"));
        }
    }
    if let Some(pj) = &host.proxy_jump {
        if !ok_single_line(pj) {
            return Err("proxy jump must not contain control characters".into());
        }
    }
    for f in &host.forwards {
        if !ok_single_line(&f.bind_addr) || !ok_single_line(&f.dest_host) {
            return Err("port-forward address must not contain control characters".into());
        }
    }
    // `extra_options` may span lines, but a lone CR would corrupt the block.
    if let Some(extra) = &host.extra_options {
        if extra.contains('\r') {
            return Err("extra options must not contain carriage returns".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn hosts_list(app: AppHandle) -> Result<Vec<Host>, String> {
    all_hosts(&app)
}

#[tauri::command]
pub fn host_save(app: AppHandle, host: Host) -> Result<String, String> {
    validate_host(&host)?;
    if host.source == HostSource::SshConfig {
        config::write_host_block(&host)?;
        Ok(host.id)
    } else {
        let store = app_store_path(&app)?;
        Ok(config::upsert_app_host(&store, host)?)
    }
}

#[tauri::command]
pub fn host_delete(app: AppHandle, host: Host) -> Result<(), String> {
    if host.source == HostSource::SshConfig {
        Ok(config::delete_host_block(&host.alias)?)
    } else {
        let store = app_store_path(&app)?;
        Ok(config::delete_app_host(&store, &host.id)?)
    }
}

#[tauri::command]
pub fn discover_history_hosts(app: AppHandle) -> Result<Vec<Host>, String> {
    let existing = all_hosts(&app)?;
    let mut found = crate::ssh::discover::discover_hosts();
    found.retain(|h| {
        !existing
            .iter()
            .any(|e| e.hostname == h.hostname && e.user == h.user)
    });
    Ok(found)
}

#[tauri::command]
pub fn ssh_build_command(app: AppHandle, host_id: String) -> Result<String, String> {
    let host = all_hosts(&app)?
        .into_iter()
        .find(|h| h.id == host_id)
        .ok_or("host not found")?;
    Ok(build_command(&host))
}

#[tauri::command]
pub fn ssh_set_passphrase(key_path: String, secret: String) -> Result<(), String> {
    Ok(keychain::set_passphrase(&key_path, &secret)?)
}

#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    state: State<'_, SshState>,
    host_id: String,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    let all = all_hosts(&app)?;
    let host = all
        .iter()
        .find(|h| h.id == host_id)
        .cloned()
        .ok_or("host not found")?;
    Ok(session::connect(state.inner(), app.clone(), host, all, cols, rows).await?)
}

#[tauri::command]
pub async fn ssh_trust_hostkey(
    state: State<'_, SshState>,
    prompt_id: String,
    trust: bool,
) -> Result<(), String> {
    if let Some(tx) = state.hostkey_prompts.lock().await.remove(&prompt_id) {
        let _ = tx.send(trust);
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_write(
    state: State<'_, SshState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    if let Some(entry) = sessions.get(&session_id) {
        entry
            .tx
            .send(SessionCmd::Write(data.into_bytes()))
            .map_err(|_| "session closed".to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_resize(
    state: State<'_, SshState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    if let Some(entry) = sessions.get(&session_id) {
        let _ = entry.tx.send(SessionCmd::Resize(cols, rows));
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_disconnect(state: State<'_, SshState>, session_id: String) -> Result<(), String> {
    session::disconnect(state.inner(), &session_id).await;
    Ok(())
}

#[tauri::command]
pub async fn forward_start(
    state: State<'_, SshState>,
    session_id: String,
    spec: ForwardSpec,
) -> Result<String, String> {
    let handle = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .map(|e| e.handle.clone())
            .ok_or("session not found")?
    };
    let fh = crate::ssh::forward::start(handle, spec).await?;
    let fwd_id = format!("fwd:{}", uuid::Uuid::new_v4());
    // If the session vanished while binding, abort the listener instead of
    // leaking it — the forward would otherwise stay bound but untracked.
    match state.sessions.lock().await.get_mut(&session_id) {
        Some(entry) => {
            entry.forwards.insert(fwd_id.clone(), fh);
        }
        None => {
            fh.task.abort();
            return Err("session closed before forward was registered".into());
        }
    }
    Ok(fwd_id)
}

#[tauri::command]
pub async fn forward_stop(
    state: State<'_, SshState>,
    session_id: String,
    forward_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(entry) = sessions.get_mut(&session_id) {
        if let Some(fh) = entry.forwards.remove(&forward_id) {
            fh.task.abort();
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardInfo {
    pub id: String,
    pub spec: ForwardSpec,
}

#[tauri::command]
pub async fn forwards_list(
    state: State<'_, SshState>,
    session_id: String,
) -> Result<Vec<ForwardInfo>, String> {
    let sessions = state.sessions.lock().await;
    let list = sessions
        .get(&session_id)
        .map(|e| {
            e.forwards
                .iter()
                .map(|(id, f)| ForwardInfo { id: id.clone(), spec: f.spec.clone() })
                .collect()
        })
        .unwrap_or_default();
    Ok(list)
}

fn build_command(host: &Host) -> String {
    let mut parts = vec!["ssh".to_string()];
    if let Some(pj) = &host.proxy_jump {
        if !pj.is_empty() {
            parts.push(format!("-J {pj}"));
        }
    }
    if let Some(id) = &host.identity_file {
        if !id.is_empty() {
            parts.push(format!("-i {id}"));
        }
    }
    if host.port != DEFAULT_SSH_PORT {
        parts.push(format!("-p {}", host.port));
    }
    for f in &host.forwards {
        parts.push(format!(
            "-L {}:{}:{}:{}",
            f.bind_addr, f.bind_port, f.dest_host, f.dest_port
        ));
    }
    let target = if host.user.is_empty() {
        host.hostname.clone()
    } else {
        format!("{}@{}", host.user, host.hostname)
    };
    parts.push(target);
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_host() -> Host {
        Host {
            alias: "web".into(),
            hostname: "example.com".into(),
            user: "deploy".into(),
            ..Host::default()
        }
    }

    #[test]
    fn validate_accepts_clean_host() {
        assert!(validate_host(&sample_host()).is_ok());
    }

    #[test]
    fn validate_rejects_newline_injection() {
        let mut host = sample_host();
        host.hostname = "example.com\n    ProxyCommand touch /tmp/pwned".into();
        assert!(validate_host(&host).is_err());
    }

    #[test]
    fn validate_rejects_whitespace_alias() {
        let mut host = sample_host();
        host.alias = "two words".into();
        assert!(validate_host(&host).is_err());
    }

    #[test]
    fn build_command_includes_non_default_port_and_user() {
        let mut host = sample_host();
        host.port = 2222;
        let cmd = build_command(&host);
        assert!(cmd.contains("-p 2222"));
        assert!(cmd.contains("deploy@example.com"));
    }
}
