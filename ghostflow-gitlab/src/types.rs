use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FullUser {
    pub username: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    pub id: u64,
}

impl User {
    pub fn for_domain(self, domain: &str) -> FullUser {
        let User {
            username,
            email,
            name,
            ..
        } = self;

        let email = email.unwrap_or_else(|| format!("{}@users.{}", username, domain));
        FullUser {
            username,
            name,
            email,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ParentProject {
    pub id: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Group {
    pub parent_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NamespaceKind {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    Group,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Namespace {
    pub id: u64,
    pub kind: NamespaceKind,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AccessLevel {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "enabled")]
    Enabled,
    #[serde(rename = "private")]
    Private,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: u64,
    pub path_with_namespace: String,
    pub ssh_url_to_repo: String,
    pub forked_from_project: Option<ParentProject>,
    pub namespace: Namespace,
    pub path: String,
    pub builds_access_level: AccessLevel,
}

#[derive(Debug, Deserialize)]
pub struct LastPipeline {
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub id: String,
    #[serde(default)]
    pub last_pipeline: Option<LastPipeline>,
}

#[derive(Debug, Deserialize)]
pub struct Issue {
    pub project_id: u64,
    pub iid: u64,
    pub web_url: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MergeRequestPipeline {
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct MergeRequest {
    pub iid: u64,
    pub source_project_id: u64,
    pub source_branch: String,
    pub target_branch: String,
    pub web_url: String,
    pub work_in_progress: bool,
    pub description: Option<String>,
    pub sha: Option<String>,
    pub pipeline: Option<MergeRequestPipeline>,
    pub force_remove_source_branch: Option<bool>,
    pub author: Author,
}

#[derive(Debug, Deserialize)]
pub struct Note {
    pub id: u64,
    pub system: bool,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub author: Author,
}

#[derive(Debug, Deserialize)]
pub struct AwardEmoji {
    pub user: Author,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum StatusState {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "scheduled")]
    Scheduled,
}

#[derive(Debug, Deserialize)]
pub struct CommitStatus {
    pub author: User,
    pub status: StatusState,
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub target_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum PipelineStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "manual")]
    Manual,
}

#[derive(Debug, Deserialize)]
pub struct Pipeline {
    pub sha: String,
    pub id: u64,
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub status: PipelineStatus,
}

#[derive(Debug, Deserialize)]
pub struct PipelineJob {
    pub status: PipelineStatus,
    pub stage: String,
    pub name: String,
    pub id: u64,
}
