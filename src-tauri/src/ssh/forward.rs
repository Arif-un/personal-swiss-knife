use std::sync::Arc;

use russh::client::Handle;
use tokio::net::TcpListener;

use crate::ssh::session::Client;
use crate::ssh::{ForwardSpec, SshError, SshResult, DEFAULT_BIND_ADDR};

pub struct ForwardHandle {
    pub spec: ForwardSpec,
    pub task: tokio::task::JoinHandle<()>,
}

/// Start a local (`-L`) port-forward: listen on `bind_addr:bind_port` and tunnel
/// each accepted connection to `dest_host:dest_port` over the SSH session.
pub async fn start(handle: Arc<Handle<Client>>, spec: ForwardSpec) -> SshResult<ForwardHandle> {
    if spec.kind != "L" {
        return Err(SshError::msg(format!(
            "unsupported forward type '{}': only local (-L) forwards are supported",
            spec.kind
        )));
    }
    let listener = TcpListener::bind((spec.bind_addr.as_str(), spec.bind_port))
        .await
        .map_err(|e| {
            SshError::msg(format!(
                "bind {}:{} failed: {e}",
                spec.bind_addr, spec.bind_port
            ))
        })?;

    let spec_for_task = spec.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let h = handle.clone();
            let dest_host = spec_for_task.dest_host.clone();
            let dest_port = spec_for_task.dest_port;
            tokio::spawn(async move {
                if let Ok(channel) = h
                    .channel_open_direct_tcpip(
                        dest_host,
                        dest_port as u32,
                        DEFAULT_BIND_ADDR.to_string(),
                        0,
                    )
                    .await
                {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                }
            });
        }
    });

    Ok(ForwardHandle { spec, task })
}
