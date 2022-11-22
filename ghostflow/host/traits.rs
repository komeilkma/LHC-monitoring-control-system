use std::error::Error;
use std::fmt;
use std::sync::Arc;

use git_workarea::{CommitId, GitContext, GitError};
use thiserror::Error;

use crate::host::pipelines::HostedPipelineService;
use crate::host::types::*;

/// Errors which may occur for a hosting service.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HostingServiceError {
    /// Failure to perform some local operation.
    #[error("service error: {}", source)]
    Service {
        /// The source of the error.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// Failure on the remote hosting service.
    #[error("host error: {}", source)]
    Host {
        /// The source of the error.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// Failure to fetch from the remote repository.
    #[error("fetch error: {}", source)]
    Fetch {
        /// The source of the error.
        #[source]
        source: GitError,
    },
    /// An unnamed commit cannot be fetched.
    #[error("cannot fetch unnamed commits")]
    UnnamedCommit {},
    /// A source repository is required.
    #[error("no source repository")]
    NoRepository {},
}

impl HostingServiceError {
    /// A convenience method for constructing a local error.
    pub fn service<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        HostingServiceError::Service {
            source: Box::new(err),
        }
    }

    /// A convenience method for constructing a remote error.
    pub fn host<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        HostingServiceError::Host {
            source: Box::new(err),
        }
    }

    /// A convenience method for constructing a fetch error.
    pub fn fetch(source: GitError) -> Self {
        HostingServiceError::Fetch {
            source,
        }
    }

    fn unnamed_commit() -> Self {
        HostingServiceError::UnnamedCommit {}
    }

    fn no_repository() -> Self {
        HostingServiceError::NoRepository {}
    }
}

/// A project hosted on a service.
#[derive(Clone)]
pub struct HostedProject {
    /// The name of the project.
    pub name: String,
    /// The service the project is hosted on.
    pub service: Arc<dyn HostingService>,
}

impl HostedProject {
    /// Get a commit for a project.
    pub fn commit(&self, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        self.service.commit(&self.name, commit)
    }

    /// Get a merge request on a project.
    pub fn merge_request(&self, id: u64) -> Result<MergeRequest, HostingServiceError> {
        self.service.merge_request(&self.name, id)
    }
}

impl fmt::Debug for HostedProject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("HostedProject")
            .field("name", &self.name)
            .finish()
    }
}

/// A hosting service.
pub trait HostingService: Send + Sync {
    /// Fetch a commit into a given git context.
    ///
    /// The default implementation requires that the commit have a valid refname, otherwise the
    /// fetch will fail.
    fn fetch_commit(&self, git: &GitContext, commit: &Commit) -> Result<(), HostingServiceError> {
        if let Some(ref refname) = commit.refname {
            git.fetch(&commit.repo.url, [refname])
                .map_err(HostingServiceError::fetch)
        } else {
            Err(HostingServiceError::unnamed_commit())
        }
    }
    /// Fetch a merge request into a given git context.
    fn fetch_mr(&self, git: &GitContext, mr: &MergeRequest) -> Result<(), HostingServiceError> {
        let url = if let Some(ref repo) = mr.source_repo {
            &repo.url
        } else {
            return Err(HostingServiceError::no_repository());
        };

        git.fetch(url, [&mr.source_branch])
            .map_err(HostingServiceError::fetch)
    }

    /// Get more specific hosting service access.
    fn as_pipeline_service(self: Arc<Self>) -> Option<Arc<dyn HostedPipelineService>> {
        None
    }

    /// The user the service is acting as.
    fn service_user(&self) -> &User;

    /// Get a user by name.
    ///
    /// The `project` is required because some services may have a per-project access
    /// restrictions for users.
    fn user(&self, project: &str, user: &str) -> Result<User, HostingServiceError>;
    /// Get a commit for a project.
    fn commit(&self, project: &str, commit: &CommitId) -> Result<Commit, HostingServiceError>;
    /// Get a merge request on a project.
    fn merge_request(&self, project: &str, id: u64) -> Result<MergeRequest, HostingServiceError>;
    /// Get a repository by name.
    fn repo(&self, project: &str) -> Result<Repo, HostingServiceError>;

    /// Get comments for a merge request.
    ///
    /// Comments are ordered from oldest to newest.
    fn get_mr_comments(&self, mr: &MergeRequest) -> Result<Vec<Comment>, HostingServiceError>;
    /// Add a comment to a merge request.
    fn post_mr_comment(&self, mr: &MergeRequest, content: &str) -> Result<(), HostingServiceError>;
    /// Get the latest commit statuses for a commit.
    fn get_commit_statuses(
        &self,
        commit: &Commit,
    ) -> Result<Vec<CommitStatus>, HostingServiceError>;
    /// Create a commit status.
    fn post_commit_status(&self, status: PendingCommitStatus) -> Result<(), HostingServiceError>;
    /// Create a review of a merge request.
    fn post_review(
        &self,
        status: PendingCommitStatus,
        mr: &MergeRequest,
        description: &str,
    ) -> Result<(), HostingServiceError> {
        self.post_commit_status(status)?;
        if !description.is_empty() {
            self.post_mr_comment(mr, description)?;
        }
        Ok(())
    }

    /// Get awards on a merge request.
    fn get_mr_awards(&self, mr: &MergeRequest) -> Result<Vec<Award>, HostingServiceError>;

    /// Get issues which are closed by a merge request.
    fn issues_closed_by_mr(&self, mr: &MergeRequest) -> Result<Vec<Issue>, HostingServiceError>;
    /// Add labels to an issue.
    fn add_issue_labels(&self, issue: &Issue, labels: &[&str]) -> Result<(), HostingServiceError>;
}
