use serde::{Deserialize, Deserializer, Serialize};

pub mod commands;
pub mod gh;
pub mod views;

/// The label whose add/remove toggles a CI run. Compared case-insensitively.
pub const CI_LABEL: &str = "ci";

/// PRs per batched GraphQL request (keeps a page of PRs to a few requests).
pub const GRAPHQL_CHUNK: usize = 20;
/// Per-PR node limit inspected in a batched GraphQL request.
pub const GRAPHQL_PAGE: u32 = 100;
/// Default number of PRs to list when the caller does not specify a limit.
pub const DEFAULT_LIMIT: u32 = 30;

/// A pull request as surfaced to the UI. Deserialized straight from
/// `gh pr list --json ...`; `author`/`labels` are flattened from gh's nested
/// shape via the custom deserializers below.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "de_author_login")]
    pub author: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub head_ref_name: String,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub state: String,
    #[serde(default, deserialize_with = "de_label_names")]
    pub labels: Vec<String>,
}

/// A single CI check for a PR, deserialized from `gh pr checks --json ...`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrCheck {
    #[serde(default)]
    pub name: String,
    /// Workflow the check belongs to; used to group checks in the UI. May be
    /// empty for non-Actions checks (external statuses).
    #[serde(default)]
    pub workflow: String,
    /// pass | fail | pending | skipping | cancel
    #[serde(default)]
    pub bucket: String,
    /// raw state, e.g. SUCCESS / FAILURE / PENDING / IN_PROGRESS
    #[serde(default)]
    pub state: String,
    /// URL to the check's logs/details.
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub completed_at: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrFilters {
    /// open | closed | merged | all
    pub state: Option<String>,
    pub author: Option<String>,
    pub assignee: Option<String>,
    /// comma-separated list of labels
    pub labels: Option<String>,
    pub base: Option<String>,
    pub head: Option<String>,
    /// raw GitHub search query (full search syntax)
    pub search: Option<String>,
    pub draft_only: Option<bool>,
    pub limit: Option<u32>,
}

/// Unified error type for the GitHub layer. Converted to a `String` at the Tauri
/// command boundary via `impl From<GithubError> for String`, so command bodies
/// can return `Result<T, String>` and use `?` directly on these errors.
#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("failed to run gh CLI: {0}")]
    Spawn(String),
    #[error("gh command failed: {0}")]
    Command(String),
    #[error("failed to parse gh output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

impl From<GithubError> for String {
    fn from(e: GithubError) -> Self {
        e.to_string()
    }
}

pub type GithubResult<T> = std::result::Result<T, GithubError>;

/// Deserialize gh's `author: { login, ... }` object down to the login string.
fn de_author_login<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Author {
        #[serde(default)]
        login: String,
    }
    Ok(Option::<Author>::deserialize(d)?
        .map(|a| a.login)
        .unwrap_or_default())
}

/// Deserialize gh's `labels: [{ name, ... }]` array down to the name strings.
fn de_label_names<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Label {
        #[serde(default)]
        name: String,
    }
    Ok(Option::<Vec<Label>>::deserialize(d)?
        .unwrap_or_default()
        .into_iter()
        .map(|l| l.name)
        .collect())
}
