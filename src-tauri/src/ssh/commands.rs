use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::ssh::session::{SessionCmd, SshState};
use crate::ssh::{config, keychain, session, ForwardSpec, Host};

fn app_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("hosts.json"))
}

fn all_hosts(app: &AppHandle) -> Result<Vec<Host>, String> {
    let mut hosts = config::parse_ssh_config();
    let store = app_store_path(app)?;
    hosts.extend(config::load_app_hosts(&store).map_err(|e| e.to_string())?);
    Ok(hosts)
}

#[tauri::command]
pub fn hosts_list(app: AppHandle) -> Result<Vec<Host>, String> {
    all_hosts(&app)
}

#[tauri::command]
pub fn host_save(app: AppHandle, host: Host) -> Result<String, String> {
    if host.source == "ssh-config" {
        config::write_host_block(&host).map_err(|e| e.to_string())?;
        Ok(host.id)
    } else {
        let store = app_store_path(&app)?;
        config::upsert_app_host(&store, host).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn host_delete(app: AppHandle, host: Host) -> Result<(), String> {
    if host.source == "ssh-config" {
        config::delete_host_block(&host.alias).map_err(|e| e.to_string())
    } else {
        let store = app_store_path(&app)?;
        config::delete_app_host(&store, &host.id).map_err(|e| e.to_string())
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
    keychain::set_passphrase(&key_path, &secret).map_err(|e| e.to_string())
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
    session::connect(state.inner(), app.clone(), host, all, cols, rows)
        .await
        .map_err(|e| e.to_string())
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
    let fh = crate::ssh::forward::start(handle, spec)
        .await
        .map_err(|e| e.to_string())?;
    let fwd_id = format!("fwd:{}", uuid::Uuid::new_v4());
    if let Some(entry) = state.sessions.lock().await.get_mut(&session_id) {
        entry.forwards.insert(fwd_id.clone(), fh);
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
    if host.port != 22 {
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
