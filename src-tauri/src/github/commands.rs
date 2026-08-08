//! Thin `#[tauri::command]` handlers for the GitHub PR feature. Each wraps the
//! `gh` service layer in `super::gh`; no subprocess/JSON plumbing lives here.

use std::collections::HashMap;

use serde::Deserialize;

use super::gh;
use super::{GithubResult, PrCheck, PrFilters, PullRequest, CI_LABEL, DEFAULT_LIMIT, GRAPHQL_PAGE};

/// Trim a filter value and drop it if empty.
fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Run a blocking `gh` operation on a background thread so a Tauri command never
/// stalls the main event loop, flattening the join error into the command's
/// `String` error channel. The closure returns `Result<T, String>`, so `?` on a
/// `GithubError` inside it converts via `From<GithubError> for String`.
async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn fetch_pull_requests(
    repo: String,
    filters: Option<PrFilters>,
) -> Result<Vec<PullRequest>, String> {
    // The `gh` call blocks on subprocess + network I/O; run it off the main
    // thread so it never stalls the UI event loop.
    run_blocking(move || {
        let f = filters.unwrap_or_default();

        let mut args: Vec<String> = vec!["pr".into(), "list".into(), "--repo".into(), repo];

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
        args.push(
            f.limit
                .filter(|n| *n > 0)
                .unwrap_or(DEFAULT_LIMIT)
                .to_string(),
        );

        args.push("--json".into());
        args.push("number,title,author,createdAt,url,isDraft,headRefName,state,labels".into());

        Ok(gh::run_gh_json(&args)?)
    })
    .await
}

/// Fetch the current label names for a single PR.
fn fetch_pr_labels(repo: &str, number: u64) -> GithubResult<Vec<String>> {
    #[derive(Deserialize)]
    struct Labels {
        #[serde(default, deserialize_with = "super::de_label_names")]
        labels: Vec<String>,
    }
    let wrap: Labels = gh::run_gh_json([
        "pr",
        "view",
        &number.to_string(),
        "--repo",
        repo,
        "--json",
        "labels",
    ])?;
    Ok(wrap.labels)
}

/// Run `gh pr edit` with a single label flag (`--add-label` / `--remove-label`).
fn edit_pr_label(repo: &str, number: u64, flag: &str, label: &str) -> GithubResult<()> {
    gh::run_gh([
        "pr",
        "edit",
        &number.to_string(),
        "--repo",
        repo,
        flag,
        label,
    ])?;
    Ok(())
}

/// Add the CI label to a PR. If it is already present, remove it first and
/// re-add it (forces a fresh label event). Returns the PR's labels afterwards.
#[tauri::command]
pub async fn readd_ci_label(repo: String, number: u64) -> Result<Vec<String>, String> {
    run_blocking(move || {
        let labels = fetch_pr_labels(&repo, number)?;
        if labels.iter().any(|l| l.eq_ignore_ascii_case(CI_LABEL)) {
            edit_pr_label(&repo, number, "--remove-label", CI_LABEL)?;
        }
        edit_pr_label(&repo, number, "--add-label", CI_LABEL)?;
        Ok(fetch_pr_labels(&repo, number)?)
    })
    .await
}

/// Count how many times the CI label was *added* (`labeled` events) to each PR.
/// Only the most recent `GRAPHQL_PAGE` label events per PR are inspected.
#[tauri::command]
pub async fn fetch_ci_label_counts(
    repo: String,
    numbers: Vec<u64>,
) -> Result<HashMap<u64, u64>, String> {
    run_blocking(move || {
        let selection = format!(
            "timelineItems(itemTypes: [LABELED_EVENT], last: {GRAPHQL_PAGE}) {{ \
               nodes {{ ... on LabeledEvent {{ label {{ name }} }} }} \
             }}"
        );
        Ok(gh::batched_pr_graphql(
            &repo,
            &numbers,
            &selection,
            |node| {
                node["timelineItems"]["nodes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter(|n| {
                                n["label"]["name"]
                                    .as_str()
                                    .map(|s| s.eq_ignore_ascii_case(CI_LABEL))
                                    .unwrap_or(false)
                            })
                            .count() as u64
                    })
                    .unwrap_or(0)
            },
        )?)
    })
    .await
}

/// Count the unresolved review threads (conversations) for each PR.
/// Only the most recent `GRAPHQL_PAGE` review threads per PR are inspected.
#[tauri::command]
pub async fn fetch_unresolved_comment_counts(
    repo: String,
    numbers: Vec<u64>,
) -> Result<HashMap<u64, u64>, String> {
    run_blocking(move || {
        let selection = format!("reviewThreads(last: {GRAPHQL_PAGE}) {{ nodes {{ isResolved }} }}");
        Ok(gh::batched_pr_graphql(
            &repo,
            &numbers,
            &selection,
            |node| {
                node["reviewThreads"]["nodes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter(|n| !n["isResolved"].as_bool().unwrap_or(false))
                            .count() as u64
                    })
                    .unwrap_or(0)
            },
        )?)
    })
    .await
}

/// Report whether each PR is currently sitting in the repo's merge queue.
#[tauri::command]
pub async fn fetch_merge_queue_status(
    repo: String,
    numbers: Vec<u64>,
) -> Result<HashMap<u64, bool>, String> {
    run_blocking(move || {
        Ok(gh::batched_pr_graphql(
            &repo,
            &numbers,
            "isInMergeQueue",
            |node| node["isInMergeQueue"].as_bool().unwrap_or(false),
        )?)
    })
    .await
}

/// Fetch the CI checks for a single PR via `gh pr checks`. `gh pr checks` exits
/// non-zero when checks are failing, pending, or absent, so stdout is parsed
/// regardless of exit status; only a genuine failure (no JSON and not a "no
/// checks" case) surfaces an error.
#[tauri::command]
pub async fn fetch_pr_checks(repo: String, number: u64) -> Result<Vec<PrCheck>, String> {
    run_blocking(move || {
        let output = gh::run_gh_capture([
            "pr",
            "checks",
            &number.to_string(),
            "--repo",
            &repo,
            "--json",
            "name,workflow,bucket,state,link,startedAt,completedAt",
        ])?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();

        if trimmed.is_empty() {
            // No JSON came back. Distinguish "PR has no checks" from a real error.
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.to_lowercase().contains("no checks") || output.status.success() {
                return Ok(Vec::new());
            }
            return Err(format!("gh command failed: {}", stderr.trim()));
        }

        let checks: Vec<PrCheck> =
            serde_json::from_str(trimmed).map_err(|e| format!("failed to parse gh output: {e}"))?;
        Ok(checks)
    })
    .await
}
