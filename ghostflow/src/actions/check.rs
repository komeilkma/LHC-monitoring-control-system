// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `check` action.
//!
//! This action checks that commits pass to a set of git checks.

use std::fmt::{self, Debug, Write as _};
use std::sync::Arc;

use git_checks_core::{CheckResult, GitCheckConfiguration};
use git_workarea::{CommitId, GitContext};
use itertools::Itertools;
use log::{info, warn};
use thiserror::Error;

use crate::host::{
    CommitStatusState, HostingService, HostingServiceError, MergeRequest, PendingCommitStatus,
};
use crate::utils::mr::{self, CommitMergeRequestState};

/// Errors which may occur when checking a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CheckError {
    /// A commit requested to be checked is not part of the merge request.
    #[error("commit unrelated to the merge request: {}", commit)]
    UnrelatedCommit {
        /// The commit that was unrelated to the merge request.
        commit: CommitId,
    },
    /// Running the checks failed.
    #[error("running checks: {}", source)]
    RunChecks {
        /// The source of the error.
        #[from]
        source: git_checks_core::RunError,
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
    /// Failure to build result comment content.
    #[error("message building write error: {}", source)]
    BuildComment {
        /// The source of the error.
        source: std::fmt::Error,
    },
}

impl CheckError {
    fn unrelated_commit(commit: CommitId) -> Self {
        CheckError::UnrelatedCommit {
            commit,
        }
    }

    fn build_comment(source: std::fmt::Error) -> Self {
        Self::BuildComment {
            source,
        }
    }
}

type CheckActionResult<T> = Result<T, CheckError>;

/// States for a check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// The checks passed.
    Pass,
    /// The checks failed.
    Fail,
}

/// When to post about checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostWhen {
    /// Always post.
    Always,
    /// Post when failures occur.
    Failure,
}

impl PostWhen {
    fn should_post(self, state: CommitStatusState) -> bool {
        match self {
            PostWhen::Always => true,
            PostWhen::Failure => state == CommitStatusState::Failed,
        }
    }
}

impl Default for PostWhen {
    fn default() -> Self {
        PostWhen::Always
    }
}

/// Implementation of the `check` action.
pub struct Check<'a> {
    /// The context to use for checking commits.
    ctx: GitContext,
    /// The service which hosts the project.
    service: Arc<dyn HostingService>,
    /// The configuration to use for the check.
    ///
    /// This contains the actual checks to use.
    config: GitCheckConfiguration<'a>,
    /// The administrators of the project.
    ///
    /// These users are notified when checks find problems which should be brought to an
    /// administrator's attention.
    admins: &'a [String],
    /// When to post about checks.
    post_when: PostWhen,
    /// The base name for status checks.
    ///
    /// Defaults to `ghostflow`.
    base_name: String,
}

impl<'a> Check<'a> {
    /// Create a new check action.
    pub fn new(
        ctx: GitContext,
        service: Arc<dyn HostingService>,
        config: GitCheckConfiguration<'a>,
        admins: &'a [String],
    ) -> Self {
        Check {
            ctx,
            service,
            config,
            admins,
            post_when: PostWhen::default(),
            base_name: "ghostflow".into(),
        }
    }

    /// Set when to post statuses and comments.
    pub fn post_when(mut self, post_when: PostWhen) -> Self {
        self.post_when = post_when;
        self
    }

    /// Set the base name for status checks.
    pub fn base_name<B>(mut self, base_name: B) -> Self
    where
        B: Into<String>,
    {
        self.base_name = base_name.into();
        self
    }

    /// The name of the status the `check` action will use for the given branch.
    pub fn status_name<B>(&self, branch: B) -> String
    where
        B: AsRef<str>,
    {
        format!("{}-check-{}", self.base_name, branch.as_ref())
    }

    /// The description of the status the `check` action will use for the given branch.
    fn status_description<B>(branch: B, commit: &CommitId) -> String
    where
        B: AsRef<str>,
    {
        format!(
            "overall branch status for the content checks against {}\n\
             \n\
             Branch-at: {}",
            branch.as_ref(),
            commit,
        )
    }

    /// Check a range of commits.
    pub fn check_mr<R>(
        self,
        reason: R,
        base: &CommitId,
        mr: &MergeRequest,
    ) -> CheckActionResult<CheckStatus>
    where
        R: AsRef<str>,
    {
        self.check_mr_with(reason.as_ref(), base, mr, &mr.commit.id)
    }

    /// Check a range of commits from a given commit.
    ///
    /// This allows checking a merge request against a backport branch and checking only the
    /// commits which belong on the backport branch.
    pub fn check_mr_with<R>(
        self,
        reason: R,
        base: &CommitId,
        mr: &MergeRequest,
        commit_id: &CommitId,
    ) -> CheckActionResult<CheckStatus>
    where
        R: AsRef<str>,
    {
        self.check_mr_impl(reason.as_ref(), base, mr, commit_id)
    }

    /// Check a range of commits.
    ///
    /// This is the actual implementation of the check action.
    fn check_mr_impl(
        self,
        reason: &str,
        base: &CommitId,
        mr: &MergeRequest,
        commit_id: &CommitId,
    ) -> CheckActionResult<CheckStatus> {
        info!(
            target: "ghostflow/check",
            "checking merge request {}",
            mr.url,
        );

        // Ensure that the commit is part of the merge request.
        let commit_state = mr::commit_state(&self.ctx, mr, commit_id, base)?;
        let is_merged = match commit_state {
            CommitMergeRequestState::OnMergeRequest => false,
            CommitMergeRequestState::OnTarget => true,
            CommitMergeRequestState::Unrelated => {
                // Explicitly do not contact the hosting service here; the caller should collect
                // this information and handle it.
                return Err(CheckError::unrelated_commit(commit_id.clone()));
            },
        };

        let status_name = self.status_name(base.as_str());
        let status_description = Self::status_description(base.as_str(), commit_id);

        if self.post_when.should_post(CommitStatusState::Pending) {
            let status = mr.create_commit_status(
                CommitStatusState::Pending,
                &status_name,
                &status_description,
            );
            if let Err(err) = self.service.post_commit_status(status) {
                warn!(
                    target: "ghostflow/check",
                    "failed to post pending commit status to {}: {:?}",
                    mr.url, err,
                );
            }
        }

        let topic_checks =
            self.config
                .run_topic(&self.ctx, reason, base, commit_id, &mr.author.identity())?;
        let mut result: CheckResult = topic_checks.into();

        if mr.work_in_progress {
            result.add_warning("the merge request is marked as a work-in-progress.");
        }
        if is_merged {
            result.add_error(format!(
                "the merge request is already merged into {}.",
                base,
            ));
        }

        let state = if result.allowed() || result.pass() {
            CommitStatusState::Success
        } else {
            CommitStatusState::Failed
        };
        let status = mr.create_commit_status(state, &status_name, &status_description);

        self.report_to_mr(mr, status, result)
    }

    /// Post the results of a check as a merge request comment.
    fn report_to_mr(
        &self,
        mr: &MergeRequest,
        status: PendingCommitStatus,
        result: CheckResult,
    ) -> CheckActionResult<CheckStatus> {
        // Just silently accept allowed MRs.
        Ok(if result.allowed() {
            if self.post_when.should_post(status.state) {
                self.service.post_commit_status(status)?;
            }

            CheckStatus::Pass
        } else {
            let pass = result.pass();

            if self.post_when.should_post(status.state) {
                let comment = self.check_result_comment(result, true)?;
                self.service.post_review(status, mr, &comment)?;
            }

            if pass {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            }
        })
    }

    /// Create a comment for the given check result.
    fn check_result_comment(
        &self,
        result: CheckResult,
        with_assist: bool,
    ) -> CheckActionResult<String> {
        let mut comment = String::new();

        // This scope is necessary so that the borrow in `push_results` ends before we use
        // `comment` again at the end of the function.
        {
            let mut push_results = |label, items: &Vec<String>| {
                if !items.is_empty() {
                    comment.push_str(&Self::comment_fragment(label, items));
                }
            };

            push_results("Errors", result.errors());
            push_results("Warnings", result.warnings());
            push_results("Alerts", result.alerts());
        }

        if with_assist {
            if !result.warnings().is_empty() {
                comment.push_str(
                    "The warnings do not need to be fixed, but it is recommended to do so.\n\n",
                );
            }

            if !result.errors().is_empty() {
                comment.push_str(
                    "Please rewrite commits to fix the errors listed above (adding fixup commits \
                     will not resolve the errors) and force-push the branch again to update the \
                     merge request.\n\n",
                );
            }

            if result.temporary() {
                comment.push_str(
                    "Some messages may be temporary; please trigger the checks again if they have \
                     been resolved.\n\n",
                );
            }
        }

        if !result.alerts().is_empty() {
            write!(comment, "Alert: @{}.\n\n", self.admins.join(" @"))
                .map_err(CheckError::build_comment)?;
        }

        // Remove trailing whitespace from the comment.
        let non_ws_len = comment.trim_end().len();
        comment.truncate(non_ws_len);

        Ok(comment)
    }

    /// Create a fragment of the comment.
    fn comment_fragment(label: &str, items: &[String]) -> String {
        format!("{}:\n\n  - {}\n\n", label, items.iter().join("\n  - "))
    }
}

impl<'a> Debug for Check<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Check")
            .field("ctx", &self.ctx)
            .field("config", &self.config)
            .field("admins", &self.admins)
            .finish()
    }
}
