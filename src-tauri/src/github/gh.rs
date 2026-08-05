//! Thin service layer over the `gh` CLI: one place that spawns the process,
//! checks the exit status, and parses JSON, so command handlers stay small.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::process::{Command, Output};

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{GithubError, GithubResult, GRAPHQL_CHUNK};

/// Split `owner/name` into its parts, erroring on a malformed repo.
pub fn split_repo(repo: &str) -> GithubResult<(String, String)> {
    repo.split_once('/')
        .map(|(o, n)| (o.to_string(), n.to_string()))
        .ok_or_else(|| GithubError::Msg(format!("Invalid repo '{repo}', expected owner/name")))
}

/// Run `gh` and return its raw `Output` without inspecting the exit status.
/// Used by callers (e.g. `gh pr checks`) that must read stdout even on failure.
pub fn run_gh_capture<I, S>(args: I) -> GithubResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| GithubError::Spawn(e.to_string()))
}

/// Run `gh`, returning stdout as a string and erroring on a non-zero exit.
pub fn run_gh<I, S>(args: I) -> GithubResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_gh_capture(args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GithubError::Command(stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run `gh` and deserialize its stdout as JSON into `T`.
pub fn run_gh_json<T, I, S>(args: I) -> GithubResult<T>
where
    T: DeserializeOwned,
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let stdout = run_gh(args)?;
    Ok(serde_json::from_str(&stdout)?)
}

/// Fetch a per-PR metric for many PRs using batched, aliased GraphQL so a page
/// of PRs costs a few requests instead of one each.
///
/// `selection` is the GraphQL body selected under each `pullRequest(...)` alias;
/// `extract` maps each PR's resolved JSON node to the metric value `T`.
///
/// `owner`/`name` are passed as GraphQL variables (never string-interpolated),
/// so a repo containing `"` cannot break out of the query.
pub fn batched_pr_graphql<T, F>(
    repo: &str,
    numbers: &[u64],
    selection: &str,
    extract: F,
) -> GithubResult<HashMap<u64, T>>
where
    F: Fn(&Value) -> T,
{
    let (owner, name) = split_repo(repo)?;
    let mut out: HashMap<u64, T> = HashMap::new();

    for chunk in numbers.chunks(GRAPHQL_CHUNK) {
        if chunk.is_empty() {
            continue;
        }

        let mut fields = String::new();
        for n in chunk {
            fields.push_str(&format!(
                "p{n}: pullRequest(number: {n}) {{ {selection} }} "
            ));
        }
        let query = format!(
            "query($owner: String!, $name: String!) {{ \
               repository(owner: $owner, name: $name) {{ {fields} }} \
             }}"
        );

        let args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("owner={owner}"),
            "-f".to_string(),
            format!("name={name}"),
            "-f".to_string(),
            format!("query={query}"),
        ];
        let value: Value = run_gh_json(&args)?;

        let repo_obj = &value["data"]["repository"];
        for n in chunk {
            out.insert(*n, extract(&repo_obj[format!("p{n}")]));
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_repo_valid() {
        assert_eq!(split_repo("owner/name").unwrap(), ("owner".into(), "name".into()));
    }

    #[test]
    fn split_repo_rejects_missing_slash() {
        assert!(split_repo("noslash").is_err());
    }
}
