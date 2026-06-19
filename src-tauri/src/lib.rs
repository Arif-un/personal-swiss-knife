mod ssh;

use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
struct PullRequest {
    number: u64,
    title: String,
    author: String,
    url: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
}

#[tauri::command]
fn fetch_pull_requests(repo: String) -> Result<Vec<PullRequest>, String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--repo",
            &repo,
            "--state",
            "open",
            "--json",
            "number,title,author,createdAt,url,isDraft,headRefName",
            "--limit",
            "30",
        ])
        .output()
        .map_err(|e| format!("Failed to run gh CLI: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh command failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let prs: Vec<PullRequest> = raw
        .into_iter()
        .map(|v| PullRequest {
            number: v["number"].as_u64().unwrap_or(0),
            title: v["title"].as_str().unwrap_or("").to_string(),
            author: v["author"]["login"].as_str().unwrap_or("").to_string(),
            url: v["url"].as_str().unwrap_or("").to_string(),
            created_at: v["createdAt"].as_str().unwrap_or("").to_string(),
            head_ref_name: v["headRefName"].as_str().unwrap_or("").to_string(),
            is_draft: v["isDraft"].as_bool().unwrap_or(false),
        })
        .collect();

    Ok(prs)
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ssh::session::SshState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            fetch_pull_requests,
            ssh::commands::hosts_list,
            ssh::commands::host_save,
            ssh::commands::host_delete,
            ssh::commands::ssh_build_command,
            ssh::commands::ssh_set_passphrase,
            ssh::commands::ssh_connect,
            ssh::commands::ssh_trust_hostkey,
            ssh::commands::ssh_write,
            ssh::commands::ssh_resize,
            ssh::commands::ssh_disconnect,
            ssh::commands::forward_start,
            ssh::commands::forward_stop,
            ssh::commands::forwards_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
