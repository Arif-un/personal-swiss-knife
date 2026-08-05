mod pr_views;
mod ssh;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    labels: Vec<String>,
}

const CI_LABEL: &str = "ci";

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
    args.push("number,title,author,createdAt,url,isDraft,headRefName,state,labels".into());

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
            labels: label_names(&v["labels"]),
        })
        .collect();

    Ok(prs)
}

/// Extract label name strings from a gh `labels` JSON array.
fn label_names(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch the current label names for a single PR.
fn fetch_pr_labels(repo: &str, number: u64) -> Result<Vec<String>, String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &number.to_string(),
            "--repo",
            repo,
            "--json",
            "labels",
        ])
        .output()
        .map_err(|e| format!("Failed to run gh CLI: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh command failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {e}"))?;
    Ok(label_names(&v["labels"]))
}

/// Run `gh pr edit` with a single label flag (`--add-label` / `--remove-label`).
fn edit_pr_label(repo: &str, number: u64, flag: &str, label: &str) -> Result<(), String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "edit",
            &number.to_string(),
            "--repo",
            repo,
            flag,
            label,
        ])
        .output()
        .map_err(|e| format!("Failed to run gh CLI: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh command failed: {stderr}"));
    }
    Ok(())
}

/// Add the CI label to a PR. If it is already present, remove it first and
/// re-add it (forces a fresh label event). Returns the PR's labels afterwards.
#[tauri::command]
fn readd_ci_label(repo: String, number: u64) -> Result<Vec<String>, String> {
    let labels = fetch_pr_labels(&repo, number)?;
    if labels.iter().any(|l| l == CI_LABEL) {
        edit_pr_label(&repo, number, "--remove-label", CI_LABEL)?;
    }
    edit_pr_label(&repo, number, "--add-label", CI_LABEL)?;
    fetch_pr_labels(&repo, number)
}

/// Count how many times the CI label was *added* (`labeled` events) to each PR.
/// Returns a map of PR number -> add count. PRs are batched into aliased GraphQL
/// fields so a whole page of PRs costs only a few requests instead of one each.
/// Note: only the most recent 100 label events per PR are inspected.
#[tauri::command]
fn fetch_ci_label_counts(
    repo: String,
    numbers: Vec<u64>,
) -> Result<HashMap<u64, u64>, String> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| format!("Invalid repo '{repo}', expected owner/name"))?;

    let mut counts: HashMap<u64, u64> = HashMap::new();

    for chunk in numbers.chunks(20) {
        if chunk.is_empty() {
            continue;
        }

        let mut fields = String::new();
        for n in chunk {
            fields.push_str(&format!(
                "p{n}: pullRequest(number: {n}) {{ \
                   timelineItems(itemTypes: [LABELED_EVENT], first: 100) {{ \
                     nodes {{ ... on LabeledEvent {{ label {{ name }} }} }} \
                   }} \
                 }} "
            ));
        }
        let query =
            format!("query {{ repository(owner: \"{owner}\", name: \"{name}\") {{ {fields} }} }}");

        let output = Command::new("gh")
            .args(["api", "graphql", "-f"])
            .arg(format!("query={query}"))
            .output()
            .map_err(|e| format!("Failed to run gh CLI: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("gh command failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let v: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        let repo_obj = &v["data"]["repository"];
        for n in chunk {
            let count = repo_obj[format!("p{n}")]["timelineItems"]["nodes"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter(|node| {
                            node["label"]["name"]
                                .as_str()
                                .map(|s| s.eq_ignore_ascii_case(CI_LABEL))
                                .unwrap_or(false)
                        })
                        .count() as u64
                })
                .unwrap_or(0);
            counts.insert(*n, count);
        }
    }

    Ok(counts)
}

/// Count the unresolved review threads (conversations) for each PR. Returns a
/// map of PR number -> unresolved count. PRs are batched into aliased GraphQL
/// fields so a whole page of PRs costs only a few requests instead of one each.
/// Note: only the most recent 100 review threads per PR are inspected.
#[tauri::command]
fn fetch_unresolved_comment_counts(
    repo: String,
    numbers: Vec<u64>,
) -> Result<HashMap<u64, u64>, String> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| format!("Invalid repo '{repo}', expected owner/name"))?;

    let mut counts: HashMap<u64, u64> = HashMap::new();

    for chunk in numbers.chunks(20) {
        if chunk.is_empty() {
            continue;
        }

        let mut fields = String::new();
        for n in chunk {
            fields.push_str(&format!(
                "p{n}: pullRequest(number: {n}) {{ \
                   reviewThreads(first: 100) {{ \
                     nodes {{ isResolved }} \
                   }} \
                 }} "
            ));
        }
        let query =
            format!("query {{ repository(owner: \"{owner}\", name: \"{name}\") {{ {fields} }} }}");

        let output = Command::new("gh")
            .args(["api", "graphql", "-f"])
            .arg(format!("query={query}"))
            .output()
            .map_err(|e| format!("Failed to run gh CLI: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("gh command failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let v: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        let repo_obj = &v["data"]["repository"];
        for n in chunk {
            let count = repo_obj[format!("p{n}")]["reviewThreads"]["nodes"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter(|node| !node["isResolved"].as_bool().unwrap_or(false))
                        .count() as u64
                })
                .unwrap_or(0);
            counts.insert(*n, count);
        }
    }

    Ok(counts)
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
            readd_ci_label,
            fetch_ci_label_counts,
            fetch_unresolved_comment_counts,
            pr_views::pr_views_list,
            pr_views::pr_views_save,
            pr_views::pr_views_delete,
            pr_views::pr_views_set_active,
            ssh::commands::hosts_list,
            ssh::commands::host_save,
            ssh::commands::host_delete,
            ssh::commands::discover_history_hosts,
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
