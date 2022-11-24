//! The `merge` action.
//!
//! This action performs the merge of a merge request topic into the target branch. It gathers
//! information from the merge request such as reviewers, testers, acceptance or rejection
//! messages, and more to determine the resulting merge commit message.
//!
//! There are multiple implementations of it which allow handling of more complicated merge
//! strategies.

use git_workarea::{CommitId, GitError, WorkAreaError};
use itertools::Itertools;
use thiserror::Error;

use crate::host::HostingServiceError;
use crate::utils::mr;

/// Errors which may occur when merging a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MergeError {
    /// A merge request has been requested to be merged into a branch twice.
    #[error("duplicate target branch name: {}", branch)]
    DuplicateTargetBranch {
        /// The name of the branch.
        branch: String,
    },
    /// A commit requested to be merged is not part of the merge request.
    #[error("commit unrelated to the merge request: {}", commit)]
    UnrelatedCommit {
        /// The commit that was unrelated to the merge request.
        commit: CommitId,
    },
    /// A commit requested to be merged is not part of the merge request.
    #[error("commit is already on the target branch: {}", commit)]
    MergedCommit {
        /// The commit that was already merged.
        commit: CommitId,
    },
    /// Configurations for "into branches" are a cycle.
    #[error("into branches form a cycle")]
    CircularIntoBranches {},
    /// Could not log a merge request topic.
    #[error("failed to log commits on topic: {}", output)]
    LogTopic {
        /// Output from `git log`.
        output: String,
    },
    /// An internal error.
    ///
    /// This should not happen. Please report an issue if this is returned.
    #[error("INTERNAL ERROR: {}", source)]
    Internal {
        /// The source of the error.
        #[from]
        source: InternalMergeError,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
    /// Failure to perform a workarea operation.
    #[error("workarea error: {}", source)]
    Workarea {
        /// The source of the error.
        #[from]
        source: WorkAreaError,
    },
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
    /// Errors from internal utility functions.
    #[error("mr utilities error: {}", source)]
    Utility {
        /// The source of the error.
        #[from]
        source: mr::Error,
    },
}

impl MergeError {
    fn duplicate_target_branch(branch: String) -> Self {
        MergeError::DuplicateTargetBranch {
            branch,
        }
    }

    fn unrelated_commit(commit: CommitId) -> Self {
        MergeError::UnrelatedCommit {
            commit,
        }
    }

    fn merged_commit(commit: CommitId) -> Self {
        MergeError::MergedCommit {
            commit,
        }
    }

    fn circular_into_branches() -> Self {
        MergeError::CircularIntoBranches {}
    }

    fn log_topic(output: &[u8]) -> Self {
        MergeError::LogTopic {
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

pub(crate) type MergeResult<T> = Result<T, MergeError>;

/// Errors from the merge machinery internals.
///
/// If any of these are returned, it means there was some logic error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InternalMergeError {
    #[doc(hidden)]
    #[error("failed to track references to push: {}", branch)]
    ReferenceTracking { branch: String },
    #[doc(hidden)]
    #[error("topological sort put {} (source) after {} (target)", source_, target)]
    IntoBranchSorting { source_: String, target: String },
    #[doc(hidden)]
    #[error("failed to merge with `-s ours`: {}", merge_result)]
    MergeOurs { merge_result: String },
    #[doc(hidden)]
    #[error("leftover branches: \"{}\"", remaining.iter().format("\", \""))]
    LeftoverBranches { remaining: Vec<String> },
}

impl InternalMergeError {
    fn reference_tracking(branch: String) -> Self {
        InternalMergeError::ReferenceTracking {
            branch,
        }
    }

    #[allow(clippy::wrong_self_convention)]
    fn into_branch_sorting(source_: String, target: String) -> Self {
        InternalMergeError::IntoBranchSorting {
            source_,
            target,
        }
    }

    fn merge_ours(result: git_workarea::MergeResult) -> Self {
        InternalMergeError::MergeOurs {
            merge_result: format!("{:?}", result),
        }
    }

    fn leftover_branches<'a, I>(remaining: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        InternalMergeError::LeftoverBranches {
            remaining: remaining.into_iter().map(Into::into).collect(),
        }
    }
}

mod policy;
pub use self::policy::MergePolicy;
pub use self::policy::MergePolicyFilter;

mod settings;
pub use self::settings::IntoBranch;
pub use self::settings::MergeActionResult;
pub use self::settings::MergeSettings;
pub use self::settings::MergeTopology;

