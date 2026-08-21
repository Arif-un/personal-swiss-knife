//! Tauri commands for build & deploy. Config lives in `wpdeploy.json`; hosts are
//! resolved from the shared SSH store; remote work runs via system `ssh`/`scp`
//! and WP-CLI on the server. Long steps stream their output as `wpdeploy://log`
//! events and finish with a `wpdeploy://done` event.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::ssh::{config as ssh_config, current_user, expand_tilde, Host, HostSource};

use super::products::{products_for_repo, resolve_slug};
use super::{DoneEvent, LogLine, Product, WpDeployConfig, EVENT_DONE, EVENT_LOG};

// ------------------------------------------------------------------ config store
fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("wpdeploy.json"))
}

fn load_config(app: &AppHandle) -> Result<WpDeployConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(WpDeployConfig::default());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if data.trim().is_empty() {
        return Ok(WpDeployConfig::default());
    }
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

fn save_config(app: &AppHandle, cfg: &WpDeployConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    // Atomic write (temp + rename), mirroring devkon's store.
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ hosts
fn all_hosts(app: &AppHandle) -> Result<Vec<Host>, String> {
    let mut hosts = ssh_config::parse_ssh_config();
    let store = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("hosts.json");
    hosts.extend(ssh_config::load_app_hosts(&store).map_err(|e| e.to_string())?);
    Ok(hosts)
}

fn find_host(app: &AppHandle, id: &str) -> Result<Host, String> {
    all_hosts(app)?
        .into_iter()
        .find(|h| h.id == id)
        .ok_or_else(|| "target host not found — pick one in deploy settings".to_string())
}

// ------------------------------------------------------------------ ssh/scp plumbing
/// Shared ssh/scp options: accept a new host key (never block on a prompt) and a
/// sane connect timeout.
const SSH_COMMON: [&str; 2] = [
    "-o StrictHostKeyChecking=accept-new",
    "-o ConnectTimeout=15",
];

/// The `user@host` (app host) or bare alias (ssh-config host) target.
fn target(host: &Host) -> String {
    if host.source == HostSource::SshConfig && !host.alias.is_empty() {
        return host.alias.clone();
    }
    let user = if host.user.is_empty() {
        current_user()
    } else {
        host.user.clone()
    };
    if host.hostname.is_empty() {
        user
    } else {
        format!("{user}@{}", host.hostname)
    }
}

fn push_common(cmd: &mut Command) {
    for opt in SSH_COMMON {
        // Each SSH_COMMON entry is "-o key=value"; split so args are clean.
        let mut it = opt.splitn(2, ' ');
        if let (Some(a), Some(b)) = (it.next(), it.next()) {
            cmd.arg(a).arg(b);
        }
    }
}

/// `ssh <opts> <target> <remote_cmd>` with PATH fixed for GUI launches.
fn ssh_command(host: &Host, remote_cmd: &str) -> Command {
    let mut c = Command::new("ssh");
    push_common(&mut c);
    if host.source != HostSource::SshConfig {
        if host.port != crate::ssh::DEFAULT_SSH_PORT {
            c.arg("-p").arg(host.port.to_string());
        }
        if let Some(id) = &host.identity_file {
            if !id.is_empty() {
                c.arg("-i").arg(expand_tilde(id));
            }
        }
    }
    c.arg(target(host)).arg(remote_cmd);
    c.env("PATH", child_path());
    c
}

/// `scp <opts> <local> <target>:<remote_dest>`.
fn scp_command(host: &Host, local: &Path, remote_dest: &str) -> Command {
    let mut c = Command::new("scp");
    push_common(&mut c);
    if host.source != HostSource::SshConfig {
        if host.port != crate::ssh::DEFAULT_SSH_PORT {
            c.arg("-P").arg(host.port.to_string());
        }
        if let Some(id) = &host.identity_file {
            if !id.is_empty() {
                c.arg("-i").arg(expand_tilde(id));
            }
        }
    }
    c.arg(local).arg(format!("{}:{remote_dest}", target(host)));
    c.env("PATH", child_path());
    c
}

/// Single-quote a value for safe interpolation into a remote shell command.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Prepend common CLI dirs so `yarn`/`git`/`ssh` resolve even under a GUI launch
/// (Finder/Dock give a minimal PATH). Inherited PATH is kept as the tail.
fn child_path() -> String {
    let base = std::env::var("PATH").unwrap_or_default();
    let mut dirs = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin").to_string_lossy().into_owned());
        dirs.push(home.join(".yarn/bin").to_string_lossy().into_owned());
    }
    if !base.is_empty() {
        dirs.push(base);
    }
    dirs.join(":")
}

// ------------------------------------------------------------------ event emit
fn emit_log(app: &AppHandle, deploy_id: &str, stream: &str, line: &str) {
    let _ = app.emit(
        EVENT_LOG,
        LogLine {
            deploy_id: deploy_id.to_string(),
            stream: stream.to_string(),
            line: line.to_string(),
        },
    );
}

fn emit_done(app: &AppHandle, deploy_id: &str, ok: bool, message: &str, version: Option<String>) {
    let _ = app.emit(
        EVENT_DONE,
        DoneEvent {
            deploy_id: deploy_id.to_string(),
            ok,
            message: message.to_string(),
            version,
        },
    );
}

// ------------------------------------------------------------------ command runners
/// Run a command, streaming each stdout/stderr line as a log event. Returns the
/// full stdout on success; a non-zero exit is an error (with the step label).
fn run_streaming(
    app: &AppHandle,
    deploy_id: &str,
    step: &str,
    mut cmd: Command,
) -> Result<String, String> {
    emit_log(app, deploy_id, "step", step);
    let started = std::time::Instant::now();
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{step}: failed to start ({e})"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let acc = Arc::new(Mutex::new(String::new()));
    let (a1, id1, acc1) = (app.clone(), deploy_id.to_string(), acc.clone());
    let h_out = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            {
                let mut g = acc1.lock().unwrap();
                g.push_str(&line);
                g.push('\n');
            }
            emit_log(&a1, &id1, "out", &line);
        }
    });
    let (a2, id2) = (app.clone(), deploy_id.to_string());
    let h_err = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            emit_log(&a2, &id2, "err", &line);
        }
    });

    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = h_out.join();
    let _ = h_err.join();
    let out = acc.lock().unwrap().clone();
    if status.success() {
        emit_log(app, deploy_id, "time", &format!("{:.1}s", started.elapsed().as_secs_f64()));
        Ok(out)
    } else {
        Err(format!("{step}: command failed"))
    }
}

/// Run a command, capturing trimmed stdout. Non-zero exit -> Err (stderr).
fn run_capture(mut cmd: Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err.trim().to_string()
        };
        Err(msg)
    }
}

/// True when the remote command exits 0 (used for `wp plugin is-active`).
fn ssh_ok(host: &Host, remote_cmd: &str) -> bool {
    ssh_command(host, remote_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .collect()
}

// ------------------------------------------------------------------ deploy build helpers
/// Local `yarn` build command for groups whose zip does NOT self-build
/// (envira/soliloquy/cdn). NextGen + theme return None (their zip builds).
fn build_command_for(envira_dev: &str, group: &str, is_lite: bool) -> Option<Command> {
    let mut c = Command::new("yarn");
    match group {
        "envira" => {
            c.args(if is_lite { ["envira-lite", "build"] } else { ["envira", "build"] });
            c.current_dir(envira_dev);
        }
        "soliloquy" => {
            c.args(if is_lite { ["sol-lite", "build"] } else { ["sol", "build"] });
            c.current_dir(envira_dev);
        }
        "cdn" => {
            c.args(["build"]);
            c.current_dir(Path::new(envira_dev).join("envira-image-cdn"));
        }
        _ => return None,
    }
    c.env("PATH", child_path());
    Some(c)
}

// ------------------------------------------------------------------ commands
#[tauri::command]
pub fn wpdeploy_config_get(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<WpDeployConfig, String> {
    crate::security::require_main(&window)?;
    load_config(&app)
}

#[tauri::command]
pub fn wpdeploy_config_save(
    window: WebviewWindow,
    app: AppHandle,
    target_host_id: String,
    zip_base: String,
) -> Result<WpDeployConfig, String> {
    crate::security::require_main(&window)?;
    let mut cfg = load_config(&app)?;
    cfg.target_host_id = target_host_id.trim().to_string();
    cfg.zip_base = zip_base.trim().to_string();
    save_config(&app, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn wpdeploy_set_docroot(
    window: WebviewWindow,
    app: AppHandle,
    host_id: String,
    docroot: String,
) -> Result<WpDeployConfig, String> {
    crate::security::require_main(&window)?;
    let mut cfg = load_config(&app)?;
    let docroot = docroot.trim().trim_end_matches('/').to_string();
    if docroot.is_empty() {
        cfg.docroots.remove(host_id.trim());
    } else {
        cfg.docroots.insert(host_id.trim().to_string(), docroot);
    }
    save_config(&app, &cfg)?;
    Ok(cfg)
}

/// Clear all deploy settings (target host, zip base, docroots).
#[tauri::command]
pub fn wpdeploy_config_reset(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<WpDeployConfig, String> {
    crate::security::require_main(&window)?;
    let cfg = WpDeployConfig::default();
    save_config(&app, &cfg)?;
    Ok(cfg)
}

/// Products deployable from a submodule repo folder (for the row accordion).
#[tauri::command]
pub fn wpdeploy_products(
    window: WebviewWindow,
    envira_dev: String,
    repo: String,
) -> Result<Vec<Product>, String> {
    crate::security::require_main(&window)?;
    products_for_repo(envira_dev.trim(), repo.trim())
}

/// Scan `~/web/*` on the target host for `wp-config.php` and return candidate
/// docroots.
#[tauri::command]
pub async fn wpdeploy_detect_docroot(
    window: WebviewWindow,
    app: AppHandle,
    host_id: String,
) -> Result<Vec<String>, String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let host = find_host(&app, host_id.trim())?;
        let finder = "for d in $HOME/web/*/; do for c in \"$d\" \"${d}public_html\"; do \
             if [ -f \"$c/wp-config.php\" ]; then echo \"$c\"; fi; done; done";
        let out = run_capture(ssh_command(&host, finder))?;
        Ok(out
            .lines()
            .map(|l| l.trim().trim_end_matches('/').to_string())
            .filter(|l| !l.is_empty())
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Build (optional) + zip + upload + backup + install + activate + flush.
/// Streams progress; also emits a terminal `wpdeploy://done`.
#[tauri::command]
pub async fn wpdeploy_deploy(
    window: WebviewWindow,
    app: AppHandle,
    envira_dev: String,
    slug: String,
    build: bool,
    deploy_id: String,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let res = run_deploy(&app, envira_dev.trim(), slug.trim(), build, &deploy_id);
        match &res {
            Ok(version) => emit_done(&app, &deploy_id, true, "Deployed", version.clone()),
            Err(e) => emit_done(&app, &deploy_id, false, e, None),
        }
        res.map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run_deploy(
    app: &AppHandle,
    envira_dev: &str,
    slug: &str,
    build: bool,
    deploy_id: &str,
) -> Result<Option<String>, String> {
    if envira_dev.is_empty() {
        return Err("envira-dev path not set (top of Submodules page)".into());
    }
    let (group, is_lite) = resolve_slug(envira_dev, slug)?;
    let cfg = load_config(app)?;
    if cfg.zip_base.trim().is_empty() {
        return Err("zip base dir not set — configure it in deploy settings".into());
    }
    if cfg.target_host_id.trim().is_empty() {
        return Err("no target host selected — pick one in deploy settings".into());
    }
    let host = find_host(app, &cfg.target_host_id)?;
    let docroot = cfg
        .docroots
        .get(&host.id)
        .cloned()
        .filter(|d| !d.trim().is_empty())
        .ok_or("no docroot set for the target host — configure it in deploy settings")?;

    emit_log(app, deploy_id, "step", &format!("Target: {} ({docroot})", target(&host)));
    let rhome = run_capture(ssh_command(&host, "echo $HOME"))?;
    let rhome = rhome.trim();
    if rhome.is_empty() {
        return Err("could not resolve remote home directory".into());
    }
    let tmp_dir = format!("{rhome}/.wp-deploy-tmp");
    let backups_root = format!("{rhome}/.wp-deploy-backups");

    // 1. Optional asset build (nextgen/theme build during zip).
    if build {
        if let Some(cmd) = build_command_for(envira_dev, &group, is_lite) {
            run_streaming(app, deploy_id, &format!("Building {group} assets"), cmd)?;
        }
    }

    // 2. Zip via envira-dev's own pipeline (single source of truth).
    let out_dir = Path::new(&cfg.zip_base).join(&group);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let mut zip_cmd = Command::new("yarn");
    zip_cmd
        .args(["actions", "zip", slug])
        .arg(&out_dir)
        .current_dir(envira_dev)
        .env("PATH", child_path());
    run_streaming(app, deploy_id, &format!("Zipping {slug}"), zip_cmd)?;

    let zips: Vec<PathBuf> = if group == "theme" {
        // imagely-theme emits one zip per brand.
        let mut v: Vec<PathBuf> = std::fs::read_dir(&out_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "zip"))
            .collect();
        v.sort();
        if v.is_empty() {
            return Err("no theme zip produced".into());
        }
        v
    } else {
        let z = out_dir.join(format!("{slug}.zip"));
        if !z.is_file() {
            return Err(format!("expected zip not found: {}", z.display()));
        }
        vec![z]
    };

    // Ensure remote tmp exists once.
    run_capture(ssh_command(&host, &format!("mkdir -p {}", shq(&tmp_dir))))?;

    let mut version: Option<String> = None;
    for zip in &zips {
        let name = zip
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("bad zip file name")?;
        let remote_zip = format!("{tmp_dir}/{name}");

        // 3. Upload.
        run_streaming(
            app,
            deploy_id,
            &format!("Uploading {name}"),
            scp_command(&host, zip, &remote_zip),
        )?;

        if group == "theme" {
            run_streaming(
                app,
                deploy_id,
                "Installing theme (--force)",
                ssh_command(
                    &host,
                    &format!("cd {} && wp theme install {} --force", shq(&docroot), shq(&remote_zip)),
                ),
            )?;
            let _ = run_capture(ssh_command(&host, &format!("cd {} && wp cache flush", shq(&docroot))));
        } else {
            // 4. Backup existing plugin (rotate, keep last 2).
            backup_plugin(app, deploy_id, &host, &docroot, &backups_root, slug)?;
            // 5. Install --force.
            run_streaming(
                app,
                deploy_id,
                &format!("Installing {slug} (--force)"),
                ssh_command(
                    &host,
                    &format!("cd {} && wp plugin install {} --force", shq(&docroot), shq(&remote_zip)),
                ),
            )?;
            // 6. Activate if inactive.
            let is_active = ssh_ok(&host, &format!("cd {} && wp plugin is-active {slug}", shq(&docroot)));
            if !is_active {
                run_streaming(
                    app,
                    deploy_id,
                    &format!("Activating {slug}"),
                    ssh_command(&host, &format!("cd {} && wp plugin activate {slug}", shq(&docroot))),
                )?;
            }
            // 7. Cache flush + version read (best-effort).
            let _ = run_capture(ssh_command(&host, &format!("cd {} && wp cache flush", shq(&docroot))));
            version = run_capture(ssh_command(
                &host,
                &format!("cd {} && wp plugin get {slug} --field=version", shq(&docroot)),
            ))
            .ok()
            .filter(|v| !v.is_empty());
        }

        // Clean uploaded zip.
        let _ = run_capture(ssh_command(&host, &format!("rm -f {}", shq(&remote_zip))));
    }

    emit_log(app, deploy_id, "step", "Done");
    Ok(version)
}

fn backup_plugin(
    app: &AppHandle,
    deploy_id: &str,
    host: &Host,
    docroot: &str,
    backups_root: &str,
    slug: &str,
) -> Result<(), String> {
    let plugdir = format!("{docroot}/wp-content/plugins/{slug}");
    let exists = run_capture(ssh_command(
        host,
        &format!("test -d {} && echo yes", shq(&plugdir)),
    ))
    .unwrap_or_default();
    if !exists.contains("yes") {
        emit_log(app, deploy_id, "step", &format!("No existing {slug} — first install, no backup"));
        return Ok(());
    }
    emit_log(app, deploy_id, "step", &format!("Backing up {slug}"));
    let bdir = format!("{backups_root}/{}", sanitize(&host.alias));
    let plugins = format!("{docroot}/wp-content/plugins");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cmd = format!(
        "mkdir -p {b} && tar czf {b}/{slug}-{ts}.tar.gz -C {p} {slug} && \
         ls -1t {b}/{slug}-*.tar.gz 2>/dev/null | tail -n +3 | xargs -r rm -f",
        b = shq(&bdir),
        p = shq(&plugins),
    );
    run_capture(ssh_command(host, &cmd))?;
    Ok(())
}

/// Restore the newest backup for a plugin on the target host, then reactivate.
#[tauri::command]
pub async fn wpdeploy_rollback(
    window: WebviewWindow,
    app: AppHandle,
    slug: String,
    deploy_id: String,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let res = run_rollback(&app, slug.trim(), &deploy_id);
        match &res {
            Ok(_) => emit_done(&app, &deploy_id, true, "Rolled back", None),
            Err(e) => emit_done(&app, &deploy_id, false, e, None),
        }
        res
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run_rollback(app: &AppHandle, slug: &str, deploy_id: &str) -> Result<(), String> {
    let cfg = load_config(app)?;
    if cfg.target_host_id.trim().is_empty() {
        return Err("no target host selected".into());
    }
    let host = find_host(app, &cfg.target_host_id)?;
    let docroot = cfg
        .docroots
        .get(&host.id)
        .cloned()
        .filter(|d| !d.trim().is_empty())
        .ok_or("no docroot set for the target host")?;
    let rhome = run_capture(ssh_command(&host, "echo $HOME"))?;
    let backups_root = format!("{}/.wp-deploy-backups", rhome.trim());
    let bdir = format!("{backups_root}/{}", sanitize(&host.alias));

    let newest = run_capture(ssh_command(
        &host,
        &format!("ls -1t {}/{slug}-*.tar.gz 2>/dev/null | head -1", bdir),
    ))?;
    let newest = newest.trim();
    if newest.is_empty() {
        return Err(format!("no backup found for {slug} on this host"));
    }
    emit_log(app, deploy_id, "step", &format!("Restoring {newest}"));
    let plugins = format!("{docroot}/wp-content/plugins");
    run_streaming(
        app,
        deploy_id,
        &format!("Rolling back {slug}"),
        ssh_command(
            &host,
            &format!(
                "rm -rf {p}/{slug} && tar xzf {n} -C {p}",
                p = shq(&plugins),
                n = shq(newest),
            ),
        ),
    )?;
    let _ = run_capture(ssh_command(&host, &format!("cd {} && wp plugin activate {slug}", shq(&docroot))));
    let _ = run_capture(ssh_command(&host, &format!("cd {} && wp cache flush", shq(&docroot))));
    emit_log(app, deploy_id, "step", "Done");
    Ok(())
}
