//! Utilities related to merge requests.

use git_workarea::{CommitId, GitContext, GitError};
use thiserror::Error;

use crate::host::MergeRequest;

/// Errors which may occur in merge request utility functions.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Failure to parse a commit id.
    #[error("failed to parse the commit {}: {}", commit, output)]
    ParseCommit {
        /// The commit id given.
        commit: CommitId,
        /// Output from `git rev-parse`.
        output: String,
    },
    /// Failure to list commits in a merge request.
    #[error("failed to list commits of {} for {}: {}", commit, url, output)]
    ListCommits {
        /// The merge request commit.
        commit: CommitId,
        /// The URL of the merge request.
        url: String,
        /// The output of `git rev-list`.
        output: String,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
}

impl Error {
    fn parse_commit(commit: CommitId, output: &[u8]) -> Self {
        Error::ParseCommit {
            commit,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn list_commits(commit: CommitId, url: String, output: &[u8]) -> Self {
        Error::ListCommits {
            commit,
            url,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type UtilsResult<T> = Result<T, Error>;

/// The status of a commit given a merge request and a target branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitMergeRequestState {
    /// The merge request contains the commit.
    OnMergeRequest,
    /// The commit is already contained within the target branch.
    OnTarget,
    /// The commit is not related to the merge request.
    Unrelated,
}

/// Check that a merge request contains a given commit compared to a target branch.
pub fn commit_state(
    ctx: &GitContext,
    mr: &MergeRequest,
    commit: &CommitId,
    base: &CommitId,
) -> UtilsResult<CommitMergeRequestState> {
    let rev_parse = ctx
        .git()
        .arg("rev-parse")
        .arg(commit.as_str())
        .output()
        .map_err(|err| GitError::subcommand("rev-parse", err))?;
    if !rev_parse.status.success() {
        return Err(Error::parse_commit(commit.clone(), &rev_parse.stderr));
    }
    let commit_id = String::from_utf8_lossy(&rev_parse.stdout);

    let rev_list = ctx
        .git()
        .arg("rev-list")
        .arg(format!("^{}", base))
        .arg(mr.commit.id.as_str())
        .output()
        .map_err(|err| GitError::subcommand("rev-list", err))?;
    if !rev_list.status.success() {
        return Err(Error::list_commits(
            mr.commit.id.clone(),
            mr.url.clone(),
            &rev_list.stderr,
        ));
    }
    let commits = String::from_utf8_lossy(&rev_list.stdout);

    Ok(if commits.is_empty() {
        CommitMergeRequestState::OnTarget
    } else if commits.lines().any(|commit| commit == commit_id.trim()) {
        CommitMergeRequestState::OnMergeRequest
    } else {
        CommitMergeRequestState::Unrelated
    })
}
