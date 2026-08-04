mod ssh;

use serde::{Deserialize, Serialize};
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
    state: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PrFilters {
    /// open | closed | merged | all
    state: Option<String>,
    author: Option<String>,
    assignee: Option<String>,
    /// comma-separated list of labels
    labels: Option<String>,
    base: Option<String>,
    head: Option<String>,
    /// raw GitHub search query (full search syntax)
    search: Option<String>,
    draft_only: Option<bool>,
    limit: Option<u32>,
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
fn fetch_pull_requests(
    repo: String,
    filters: Option<PrFilters>,
) -> Result<Vec<PullRequest>, String> {
    let f = filters.unwrap_or_default();

    let mut args: Vec<String> = vec![
        "pr".into(),
        "list".into(),
        "--repo".into(),
        repo,
    ];

    args.push("--state".into());
    args.push(clean(f.state).unwrap_or_else(|| "open".into()));

    if let Some(author) = clean(f.author) {
        args.push("--author".into());
        args.push(author);
    }
    if let Some(assignee) = clean(f.assignee) {
        args.push("--assignee".into());
        args.push(assignee);
    }
    if let Some(base) = clean(f.base) {
        args.push("--base".into());
        args.push(base);
    }
    if let Some(head) = clean(f.head) {
        args.push("--head".into());
        args.push(head);
    }
    if let Some(labels) = clean(f.labels) {
        for label in labels.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            args.push("--label".into());
            args.push(label.into());
        }
    }
    if let Some(search) = clean(f.search) {
        args.push("--search".into());
        args.push(search);
    }
    if f.draft_only.unwrap_or(false) {
        args.push("--draft".into());
    }

    args.push("--limit".into());
    args.push(f.limit.filter(|n| *n > 0).unwrap_or(30).to_string());

    args.push("--json".into());
    args.push("number,title,author,createdAt,url,isDraft,headRefName,state".into());

    let output = Command::new("gh")
        .args(&args)
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
            state: v["state"].as_str().unwrap_or("").to_string(),
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
