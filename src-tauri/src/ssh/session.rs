use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use russh::client::{self, Handle};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::ssh::forward::ForwardHandle;
use crate::ssh::known_hosts::HostKeyStatus;
use crate::ssh::{
    config, current_user, expand_tilde, keychain, known_hosts, Host, SshError, SshResult,
    DEFAULT_BIND_ADDR, EVENT_SSH_CLOSED, EVENT_SSH_DATA, EVENT_SSH_HOSTKEY,
};

/// How long a connect waits for the user to answer an unknown-host-key prompt
/// before giving up (and treating the host as untrusted).
const HOSTKEY_PROMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Commands sent to a session's owning task.
pub enum SessionCmd {
    Write(Vec<u8>),
    Resize(u32, u32),
    Close,
}

pub struct SessionEntry {
    pub tx: mpsc::UnboundedSender<SessionCmd>,
    pub handle: Arc<Handle<Client>>,
    pub forwards: HashMap<String, ForwardHandle>,
}

/// Shared, Tauri-managed SSH state.
#[derive(Clone)]
pub struct SshState {
    pub sessions: Arc<Mutex<HashMap<String, SessionEntry>>>,
    pub hostkey_prompts: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

impl Default for SshState {
    fn default() -> Self {
        Self::new()
    }
}

impl SshState {
    pub fn new() -> Self {
        SshState {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            hostkey_prompts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ---- event payloads ----

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshData {
    session_id: String,
    bytes: Vec<u8>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshClosed {
    session_id: String,
    reason: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostKeyPrompt {
    prompt_id: String,
    host: String,
    fingerprint: String,
    algorithm: String,
}

// ---- russh client handler ----

pub struct Client {
    app: AppHandle,
    host: String,
    port: u16,
    prompt_id: String,
    hostkey_prompts: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

impl Client {
    fn new(
        app: AppHandle,
        host: String,
        port: u16,
        prompt_id: String,
        hostkey_prompts: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    ) -> Self {
        Client {
            app,
            host,
            port,
            prompt_id,
            hostkey_prompts,
        }
    }
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        match known_hosts::check(&self.host, self.port, server_public_key) {
            HostKeyStatus::Trusted => Ok(true),
            HostKeyStatus::Changed => Ok(false),
            HostKeyStatus::Unknown => {
                let (tx, rx) = oneshot::channel();
                self.hostkey_prompts
                    .lock()
                    .await
                    .insert(self.prompt_id.clone(), tx);
                let fingerprint = server_public_key
                    .fingerprint(russh::keys::HashAlg::Sha256)
                    .to_string();
                let algorithm = server_public_key.algorithm().as_str().to_string();
                let _ = self.app.emit(
                    EVENT_SSH_HOSTKEY,
                    HostKeyPrompt {
                        prompt_id: self.prompt_id.clone(),
                        host: self.host.clone(),
                        fingerprint,
                        algorithm,
                    },
                );
                // Bounded wait: if the user never answers, treat as untrusted and
                // drop the pending prompt so it can't leak or block forever.
                let trust = match tokio::time::timeout(HOSTKEY_PROMPT_TIMEOUT, rx).await {
                    Ok(Ok(answer)) => answer,
                    Ok(Err(_)) | Err(_) => {
                        self.hostkey_prompts.lock().await.remove(&self.prompt_id);
                        false
                    }
                };
                if trust {
                    known_hosts::learn(&self.host, self.port, server_public_key);
                }
                Ok(trust)
            }
        }
    }
}

// ---- authentication ----

fn best_hash(key: &russh::keys::PrivateKey) -> Option<russh::keys::HashAlg> {
    let alg = format!("{:?}", key.algorithm()).to_lowercase();
    if alg.contains("rsa") {
        Some(russh::keys::HashAlg::Sha512)
    } else {
        None
    }
}

async fn try_agent_auth(handle: &mut Handle<Client>, user: &str) -> bool {
    let mut agent = match russh::keys::agent::client::AgentClient::connect_env().await {
        Ok(a) => a,
        Err(_) => return false,
    };
    let identities = match agent.request_identities().await {
        Ok(ids) => ids,
        Err(_) => return false,
    };
    for id in identities {
        let key = id.public_key().into_owned();
        if let Ok(res) = handle
            .authenticate_publickey_with(user.to_string(), key, None, &mut agent)
            .await
        {
            if res.success() {
                return true;
            }
        }
    }
    false
}

async fn try_key_file(handle: &mut Handle<Client>, user: &str, path: &Path) -> SshResult<bool> {
    let pass = keychain::get_passphrase(&path.to_string_lossy());
    let key = match russh::keys::load_secret_key(path, pass.as_deref()) {
        Ok(k) => k,
        Err(_) => return Ok(false),
    };
    let hash = best_hash(&key);
    let res = handle
        .authenticate_publickey(
            user.to_string(),
            russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), hash),
        )
        .await?;
    Ok(res.success())
}

async fn authenticate(handle: &mut Handle<Client>, host: &Host) -> SshResult<()> {
    let user = if host.user.is_empty() {
        current_user()
    } else {
        host.user.clone()
    };

    if host.use_agent && try_agent_auth(handle, &user).await {
        return Ok(());
    }

    // Treat an empty identity_file (field typed then cleared in the UI, which
    // round-trips as Some("")) the same as None so default-key discovery still
    // runs instead of being silently skipped.
    let identity = host
        .identity_file
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(id) = identity {
        if try_key_file(handle, &user, &expand_tilde(id)).await? {
            return Ok(());
        }
    } else if let Some(home) = dirs::home_dir() {
        for name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
            let p = home.join(".ssh").join(name);
            if p.exists() && try_key_file(handle, &user, &p).await? {
                return Ok(());
            }
        }
    }

    Err(SshError::msg(
        "authentication failed — ssh-agent and key files were rejected",
    ))
}

// ---- connect ----

pub async fn connect(
    state: &SshState,
    app: AppHandle,
    host: Host,
    all: Vec<Host>,
    cols: u32,
    rows: u32,
) -> SshResult<String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let config = Arc::new(client::Config::default());

    let mut jump_keepalive: Option<Handle<Client>> = None;

    let mut handle: Handle<Client> = match host.proxy_jump.clone().filter(|s| !s.trim().is_empty())
    {
        Some(pj) => {
            let jhost = config::resolve_jump(&pj, &all);
            let jclient = Client::new(
                app.clone(),
                jhost.hostname.clone(),
                jhost.port,
                format!("{session_id}:jump"),
                state.hostkey_prompts.clone(),
            );
            let mut jhandle = client::connect(
                config.clone(),
                (jhost.hostname.as_str(), jhost.port),
                jclient,
            )
            .await?;
            authenticate(&mut jhandle, &jhost).await?;

            let channel = jhandle
                .channel_open_direct_tcpip(
                    host.hostname.clone(),
                    host.port as u32,
                    DEFAULT_BIND_ADDR.to_string(),
                    0,
                )
                .await?;
            let stream = channel.into_stream();

            let tclient = Client::new(
                app.clone(),
                host.hostname.clone(),
                host.port,
                session_id.clone(),
                state.hostkey_prompts.clone(),
            );
            let h = client::connect_stream(config.clone(), stream, tclient).await?;
            jump_keepalive = Some(jhandle);
            h
        }
        None => {
            let tclient = Client::new(
                app.clone(),
                host.hostname.clone(),
                host.port,
                session_id.clone(),
                state.hostkey_prompts.clone(),
            );
            client::connect(config.clone(), (host.hostname.as_str(), host.port), tclient).await?
        }
    };

    authenticate(&mut handle, &host).await?;

    let handle = Arc::new(handle);
    let channel = handle.channel_open_session().await?;
    let term = std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into());
    channel
        .request_pty(true, &term, cols, rows, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;

    let (tx, mut rx) = mpsc::unbounded_channel::<SessionCmd>();
    state.sessions.lock().await.insert(
        session_id.clone(),
        SessionEntry {
            tx,
            handle: handle.clone(),
            forwards: HashMap::new(),
        },
    );

    let app2 = app.clone();
    let sessions = state.sessions.clone();
    let sid = session_id.clone();

    tokio::spawn(async move {
        let mut channel = channel;
        let _keepalive = jump_keepalive; // hold the jump session open for the lifetime
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(SessionCmd::Write(d)) => { let _ = channel.data(&d[..]).await; }
                    Some(SessionCmd::Resize(c, r)) => { let _ = channel.window_change(c, r, 0, 0).await; }
                    Some(SessionCmd::Close) | None => { let _ = channel.eof().await; break; }
                },
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) => {
                        let _ = app2.emit(EVENT_SSH_DATA, SshData { session_id: sid.clone(), bytes: data.to_vec() });
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = app2.emit(EVENT_SSH_DATA, SshData { session_id: sid.clone(), bytes: data.to_vec() });
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                },
            }
        }
        let _ = app2.emit(
            EVENT_SSH_CLOSED,
            SshClosed {
                session_id: sid.clone(),
                reason: "disconnected".into(),
            },
        );
        // Abort any port-forward listeners so they don't outlive the session
        // (dropping a JoinHandle only detaches it, leaving the port bound).
        if let Some(mut entry) = sessions.lock().await.remove(&sid) {
            for (_, f) in entry.forwards.drain() {
                f.task.abort();
            }
        }
    });

    Ok(session_id)
}

pub async fn disconnect(state: &SshState, session_id: &str) {
    let entry = state.sessions.lock().await.remove(session_id);
    if let Some(mut entry) = entry {
        for (_, f) in entry.forwards.drain() {
            f.task.abort();
        }
        let _ = entry.tx.send(SessionCmd::Close);
        let _ = entry
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await;
    }
}
