use chrono::{DateTime, Utc};
use git_workarea::{CommitId, Identity};

/// A commit status created by a `Commit` or `MergeRequest`.
#[derive(Debug, Clone)]
pub struct PendingCommitStatus<'a> {
    /// The commit the status applies to.
    pub commit: &'a Commit,
    /// The state of the commit status.
    pub state: CommitStatusState,
    /// The name of the status check.
    pub name: &'a str,
    /// A description for the status.
    pub description: &'a str,
    /// A URL to associate with the status.
    pub target_url: Option<&'a str>,
}

/// A commit hosted on the service provider.
#[derive(Debug, Clone)]
pub struct Commit {
    /// The repository where the commit lives.
    pub repo: Repo,
    /// The refname for the commit (if available).
    pub refname: Option<String>,
    /// The object id of the commit.
    pub id: CommitId,
    /// The last pipeline ID for the commit (if any).
    pub last_pipeline: Option<u64>,
}

impl Commit {
    /// Create a commit status for the commit.
    pub fn create_commit_status<'a>(
        &'a self,
        state: CommitStatusState,
        name: &'a str,
        description: &'a str,
    ) -> PendingCommitStatus<'a> {
        PendingCommitStatus {
            commit: self,
            state,
            name,
            description,
            target_url: None,
        }
    }
}

/// A repository hosted on the service.
#[derive(Debug, Clone)]
pub struct Repo {
    /// The name of the project.
    pub name: String,

    /// The URL which should be used to fetch from the repository.
    ///
    /// Whether this uses HTTPS or SSH is dependent on the service, but it should not require
    /// interaction in order to use (whether through an SSH key or administrator privileges).
    pub url: String,

    /// The repository which this project was forked from.
    pub forked_from: Option<Box<Repo>>,
}

impl Repo {
    /// The root of the fork tree for the repository.
    pub fn fork_root(&self) -> &Self {
        self.forked_from
            .as_ref()
            .map_or(self, |parent| parent.fork_root())
    }
}

/// An issue on the service.
#[derive(Debug, Clone)]
pub struct Issue {
    /// The source repository for the issue.
    pub repo: Repo,
    /// The internal ID of the issue.
    ///
    /// Service-specific.
    pub id: u64,
    /// The URL of the issue.
    pub url: String,
    /// The labels for the issue.
    pub labels: Vec<String>,
    /// A string which may be used in a comment to refer to the issue.
    pub reference: String,
}

/// The status of the hooks having been run on a merge request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// The checks passed.
    Pass,
    /// The checks failed.
    Fail,
    /// The checks have not been run.
    Unchecked,
}

impl CheckStatus {
    /// Whether the checks have been run or not.
    pub fn is_checked(self) -> bool {
        match self {
            CheckStatus::Pass | CheckStatus::Fail => true,
            CheckStatus::Unchecked => false,
        }
    }

    /// Whether the checks need to be run or not.
    pub fn is_ok(self) -> bool {
        match self {
            CheckStatus::Pass => true,
            CheckStatus::Fail | CheckStatus::Unchecked => false,
        }
    }
}

/// A merge request on the service.
#[derive(Debug, Clone)]
pub struct MergeRequest {
    /// The source repository for the merge request.
    ///
    /// If the source repository is deleted or otherwise inaccessible, it is `None`.
    pub source_repo: Option<Repo>,
    /// The name of the branch requested for merging.
    pub source_branch: String,
    /// The repository the merge request will be merged into.
    pub target_repo: Repo,
    /// The target branch for the request.
    pub target_branch: String,
    /// The internal ID of the merge request.
    ///
    /// Service-specific.
    pub id: u64,
    /// The URL of the merge request.
    pub url: String,
    /// Whether the merge request is a "work-in-progress" or not.
    pub work_in_progress: bool,
    /// The description for the merge request.
    pub description: String,
    /// The previous commit of the merge request (if available).
    ///
    /// This is particularly important for the stage action. Not so important otherwise.
    pub old_commit: Option<Commit>,
    /// The commit which has been requested for merging.
    pub commit: Commit,
    /// The author of the merge request.
    pub author: User,
    /// A string which may be used in a comment to refer to the merge request.
    pub reference: String,
    /// Whether the source branch should be removed when merging.
    pub remove_source_branch: bool,
}

impl MergeRequest {
    /// Create a commit status for the merge request.
    pub fn create_commit_status<'a>(
        &'a self,
        state: CommitStatusState,
        name: &'a str,
        description: &'a str,
    ) -> PendingCommitStatus<'a> {
        PendingCommitStatus {
            commit: &self.commit,
            state,
            name,
            description,
            target_url: None,
        }
    }
}

/// States for a commit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStatusState {
    /// The check is expected, but has not started yet.
    Pending,
    /// The check is currently running.
    Running,
    /// The check is a success.
    Success,
    /// The check is a failure.
    Failed,
}

impl From<CommitStatusState> for CheckStatus {
    fn from(state: CommitStatusState) -> Self {
        match state {
            CommitStatusState::Success => CheckStatus::Pass,
            CommitStatusState::Failed => CheckStatus::Fail,
            CommitStatusState::Pending | CommitStatusState::Running => CheckStatus::Unchecked,
        }
    }
}

/// A commit status for a specific commit.
#[derive(Debug, Clone)]
pub struct CommitStatus {
    /// The state of the commit status.
    pub state: CommitStatusState,
    /// The author of the commit status.
    pub author: User,
    /// The refname of the commit (if applicable).
    pub refname: Option<String>,
    /// The name of the check being performed.
    pub name: String,
    /// A description of the check.
    pub description: String,
    /// Target url (if available).
    pub target_url: Option<String>,
}

/// A user on the service.
#[derive(Debug, Clone)]
pub struct User {
    /// The username on the service of the user (used for mentioning the user).
    pub handle: String,
    /// The real name of the user.
    pub name: String,
    /// The email address of the user.
    pub email: String,
}

impl User {
    /// Convenience method for getting an identity for the user.
    pub fn identity(&self) -> Identity {
        Identity::new(&self.name, &self.email)
    }
}

/// A comment on the service.
#[derive(Debug, Clone)]
pub struct Comment {
    /// The ID of the note.
    pub id: String,
    /// Indicates whether the comment is autogenerated (via activity or mentions) or not.
    pub is_system: bool,
    /// Indicates whether the comment indicates a branch update or not.
    ///
    /// This is used to separate the comment stream into before and after for an update to its
    /// source topic.
    pub is_branch_update: bool,
    /// When the comment was created.
    pub created_at: DateTime<Utc>,
    /// The author of the comment.
    pub author: User,
    /// The content of the comment.
    pub content: String,
}

/// An award on the service.
#[derive(Debug, Clone)]
pub struct Award {
    /// The name of the award.
    pub name: String,
    /// The author of the award.
    pub author: User,
}
