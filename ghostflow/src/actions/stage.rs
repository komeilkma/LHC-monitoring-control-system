

//! The `stage` action.
//!
//! This action is intended to manage a temporary integration branch (normally `stage`) to perform
//! testing on a collection of branches which are on their way into the main integration branch.

use std::borrow::Cow;

use chrono::{DateTime, Utc};
use git_topic_stage::{
    CandidateTopic, IntegrationResult, Stager, StagerError, Topic, UnstageReason,
};
use git_workarea::{GitError, Identity, MergeStatus};
use itertools::Itertools;
use log::{error, info, warn};
use thiserror::Error;

use crate::host::{Commit, CommitStatusState, HostedProject, HostingServiceError, MergeRequest};

/// Operations on a stage ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageRefOp {
    /// Updating the main stage ref.
    Update,
    /// Pushing the main stage ref.
    Push,
    /// Tagging the stage.
    Tag,
    /// Pushing a tracking ref for the latest tag.
    Tracking,
    /// Pushing a tracking ref for the latest tag.
    ///
    /// Tag updates are pushed with their reason ref as well, so its name is included here.
    PushTag {
        /// The name of the tracking ref.
        tracking_ref: String,
    },
}

impl StageRefOp {
    fn desc(&self) -> &'static str {
        match self {
            StageRefOp::Update => "update stage ref",
            StageRefOp::Push => "push stage ref",
            StageRefOp::Tag => "tag stage ref",
            StageRefOp::Tracking => "update stage tracking ref",
            StageRefOp::PushTag {
                ..
            } => "push stage tag refs",
        }
    }
}

/// Errors which may occur when staging a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StageError {
    /// Failure to operate on the stage ref.
    #[error("failed to {} `{}`: {}", op.desc(), refname, output)]
    StageRef {
        /// The name of the stage ref.
        refname: String,
        /// The operation.
        op: StageRefOp,
        /// Output from `git update-ref`.
        output: String,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
    /// Failure to perform a stage operation.
    #[error("stager error: {}", source)]
    Stager {
        /// The source of the error.
        #[from]
        source: StagerError,
    },
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
}

impl StageError {
    fn stage_ref(refname: String, op: StageRefOp, output: &[u8]) -> Self {
        StageError::StageRef {
            refname,
            op,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type StageResult<T> = Result<T, StageError>;

/// Policies which may be used when tagging the stage.
#[derive(Debug, Clone, Copy)]
pub enum TagStagePolicy {
    /// Topics currently on the stage may stay.
    KeepTopics,
    /// The stage is cleared of all topics and reset to the target branch.
    ClearStage,
}

impl Default for TagStagePolicy {
    fn default() -> Self {
        TagStagePolicy::ClearStage
    }
}

/// Implementation of the `stage` action.
///
/// The stage is a collection of topic branches which should be tested together. The stage is meant
/// to be "tagged" on a regular basis and pushed for testing. In the meantime, topics may be added
/// to and removed from the staging branch. If any topic is updated, it is removed from the stage
/// and put at the end of the set of topics ready for merging. Additionally, if the base of the
/// stage updates, the entire stage is recreated.
#[derive(Debug)]
pub struct Stage {
    /// The target branch for the stage.
    branch: String,
    /// The stager manager for the branch.
    stager: Stager,
    /// The project of the target branch.
    project: HostedProject,
    /// Whether the action should create informational comments or not.
    ///
    /// Errors always create comments.
    quiet: bool,
}

impl Stage {
    /// Create a new stage action.
    pub fn new<B>(stager: Stager, branch: B, project: HostedProject) -> StageResult<Self>
    where
        B: Into<String>,
    {
        let stage = Self {
            branch: branch.into(),
            stager,
            project,
            quiet: false,
        };

        stage.update_head_ref()?;

        Ok(stage)
    }

    /// Reduce the number of comments made by the stage action.
    ///
    /// The comments created by this action can be a bit much. This reduces the comments to those
    /// which are errors or are important.
    pub fn quiet(&mut self) -> &mut Self {
        self.quiet = true;
        self
    }

    /// A reference to the internal stager.
    pub fn stager(&self) -> &Stager {
        &self.stager
    }

    /// Update the base commit for the stage.
    ///
    /// Note that this function does no checking to ensure that the given commit is related to the
    /// existing base commit.
    pub fn base_branch_update(
        &mut self,
        commit: &Commit,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> StageResult<()> {
        info!(
            target: "ghostflow/stage",
            "updating the base commit for {}/{}",
            self.project.name,
            self.branch,
        );

        // Fetch the commit into the stager's git context.
        self.project
            .service
            .fetch_commit(self.stager.git_context(), commit)?;

        let candidate = CandidateTopic {
            old_id: Some(Topic::new(
                self.stager.base().clone(),
                who.clone(),
                Utc::now(),
                0,
                "base",
                "url",
            )),
            new_id: Topic::new(commit.id.clone(), who.clone(), when, 0, "base", "url"),
        };

        self.update_stage_base(candidate)?;
        self.update_head_ref()
    }

    /// Add a merge request to the stage.
    fn stage_merge_request_impl(
        &mut self,
        mr: &MergeRequest,
        topic_name: &str,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> StageResult<()> {
        info!(
            target: "ghostflow/stage",
            "attempting to stage {}",
            mr.url,
        );

        // Fetch the MR commit into the stager's git context.
        self.project
            .service
            .fetch_mr(self.stager.git_context(), mr)?;

        let old_commit = if let Some(staged) = self.stager.find_topic_by_id(mr.id) {
            if &mr.commit.id == staged.commit() {
                self.send_info_mr_comment(
                    mr,
                    "This topic has already been staged; ignoring the request to stage.",
                );

                return Ok(());
            }

            let expected = match mr.old_commit {
                Some(ref old_commit) => {
                    let expected = staged.commit();

                    if &old_commit.id != expected {
                        warn!(
                            target: "ghostflow/stage",
                            "it appears as though an update for the merge request {} was missed; \
                             munging the request so that it removes the stale branch ({}) from the \
                             stage instead of the indicated branch ({}).",
                            mr.url,
                            expected,
                            old_commit.id,
                        );
                    }

                    expected
                },
                None => staged.commit(),
            };

            Some(Commit {
                repo: mr.commit.repo.clone(),
                refname: mr.commit.refname.clone(),
                id: expected.clone(),
                last_pipeline: mr.commit.last_pipeline,
            })
        } else {
            None
        };

        // Create the candidate topic for the MR.
        let old_hosted_commit = old_commit.as_ref();
        let candidate = CandidateTopic {
            old_id: old_hosted_commit.map(|c| {
                Topic::new(
                    c.id.clone(),
                    who.clone(),
                    Utc::now(),
                    mr.id,
                    topic_name,
                    &mr.url,
                )
            }),
            new_id: Topic::new(
                mr.commit.id.clone(),
                who.clone(),
                when,
                mr.id,
                topic_name,
                &mr.url,
            ),
        };

        // Update the stage.
        self.update_stage_mr(candidate, old_hosted_commit, mr)?;
        // Push the new stage state to the remote.
        self.update_head_ref()
    }

    /// Add a merge request to the stage with a given name.
    pub fn stage_merge_request_named<N>(
        &mut self,
        mr: &MergeRequest,
        name: N,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> StageResult<()>
    where
        N: AsRef<str>,
    {
        self.stage_merge_request_impl(mr, name.as_ref(), who, when)
    }

    /// Add a merge request to the stage.
    pub fn stage_merge_request(
        &mut self,
        mr: &MergeRequest,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> StageResult<()> {
        self.stage_merge_request_impl(mr, &mr.source_branch, who, when)
    }

    /// Unstage a merge request.
    fn unstage_merge_request_impl(
        &mut self,
        mr: &MergeRequest,
        success_msg: &str,
        missing_msg: Option<&str>,
    ) -> StageResult<()> {
        let staged_topic_opt = self.stager.find_topic_by_id(mr.id).cloned();

        if let Some(staged_topic) = staged_topic_opt {
            let stage_result = self.stager.unstage(staged_topic)?;

            self.send_info_mr_comment(
                mr,
                &format!("This merge request has been unstaged {}.", success_msg),
            );
            self.send_mr_commit_status(mr, CommitStatusState::Success, "unstaged");

            // Update topics have been punted off of the stage (successfully staged commits are
            // fine).
            for topic in &stage_result.results {
                self.update_mr_state(topic, false, &mr_update_reason(mr))?;
            }

            // Push the new stage state to the remote.
            self.update_head_ref()?
        } else if let Some(msg) = missing_msg {
            self.send_info_mr_comment(mr, msg)
        }

        Ok(())
    }

    /// Remove a merge request from the stage due to an update.
    pub fn unstage_update_merge_request(
        &mut self,
        mr: &MergeRequest,
        reason: &str,
    ) -> StageResult<()> {
        info!(
            target: "ghostflow/stage",
            "attempting to unstage {} because of an update {}",
            mr.url,
            reason,
        );

        self.unstage_merge_request_impl(mr, reason, None)
    }

    /// Remove a merge request from the stage.
    pub fn unstage_merge_request(&mut self, mr: &MergeRequest) -> StageResult<()> {
        info!(
            target: "ghostflow/stage",
            "attempting to unstage {}",
            mr.url,
        );

        self.unstage_merge_request_impl(
            mr,
            "upon request",
            Some(
                "Failed to find this merge request on the stage; ignoring the request to unstage \
                 it.",
            ),
        )
    }

    /// Tag the stage into a ref and reset the state of the stage.
    ///
    /// The ref `refs/stage/{branch}/{reason}/latest` and
    /// `refs/stage/{branch}/{reason}/{dateformat}` are updated to point to the current state of
    /// the stage.
    pub fn tag_stage(
        &mut self,
        reason: &str,
        ref_date_format: &str,
        policy: TagStagePolicy,
    ) -> StageResult<()> {
        info!(
            target: "ghostflow/stage",
            "tagging the stage for {}/{}",
            self.project.name,
            self.branch,
        );

        // Tag the current state of the stage.
        let (when, stage_ref) = self.tag_latest_ref(reason, ref_date_format)?;

        let (staged_topics, msg) = match policy {
            TagStagePolicy::ClearStage => {
                let msg = format!(
                    "This merge request has been pushed for {} testing as of {} and unstaged.",
                    reason,
                    when.format(ref_date_format),
                );
                (Cow::Owned(self.stager.clear()), Some(msg))
            },
            TagStagePolicy::KeepTopics => (Cow::Borrowed(self.stager.topics()), None),
        };
        let state_desc = format!("staged for {} testing {}", reason, stage_ref);
        for staged_topic in staged_topics.iter() {
            let mr_res = self.hosted_mr(&staged_topic.topic);
            match mr_res {
                Ok(mr) => {
                    self.send_mr_commit_status(&mr, CommitStatusState::Success, &state_desc);

                    if let Some(msg) = msg.as_ref() {
                        self.send_mr_comment(&mr, msg);
                    }
                },
                Err(err) => {
                    error!(
                        target: "ghostflow/stage",
                        "failed to fetch mr {} for {}: {:?}",
                        staged_topic.topic.id,
                        self.project.name,
                        err,
                    );
                },
            }
        }

        // Push the new stage to the remote.
        self.update_head_ref()
    }

    /// Update the base of the stage.
    fn update_stage_base(&mut self, candidate: CandidateTopic) -> StageResult<()> {
        let stage_result = self.stager.stage(candidate)?;

        // Update topics have been punted off of the stage (successfully staged commits are fine).
        for topic in &stage_result.results {
            self.update_mr_state(
                topic,
                false,
                &format!("an update to the {} branch causing ", self.branch),
            )?;
        }

        Ok(())
    }

    /// Update a merge request which is already on the stage.
    fn update_stage_mr(
        &mut self,
        candidate: CandidateTopic,
        old_commit: Option<&Commit>,
        mr: &MergeRequest,
    ) -> StageResult<()> {
        let stage_result = self.stager.stage(candidate)?;

        // We use success here because it was successfully unstaged. A failure would cause a old
        // commits to never be shown as "passing" where this information might be useful at a
        // glance.
        if let Some(commit) = old_commit {
            self.send_commit_status(commit, CommitStatusState::Success, "unstaged");
        }

        // If no results were made, the base branch was updated and no topics were already staged;
        // everything is fine.
        let results = stage_result.results[..].split_last();
        if let Some((new_topic, restaged_topics)) = results {
            let update_reason = mr_update_reason(mr);

            // Update topics have been punted off of the stage (successfully staged commits are
            // fine).
            for topic in restaged_topics {
                self.update_mr_state(topic, false, &update_reason)?;
            }

            self.update_mr_state(new_topic, true, "")?;
        }

        Ok(())
    }

    /// Update the `HEAD` ref of the stage.
    fn update_head_ref(&self) -> StageResult<()> {
        let ctx = self.stager.git_context();
        let refname = format!("refs/stage/{}/head", self.branch);

        let update_ref = ctx
            .git()
            .arg("update-ref")
            .arg(&refname)
            .arg(self.stager.head().as_str())
            .output()
            .map_err(|err| GitError::subcommand("update-ref", err))?;
        if !update_ref.status.success() {
            return Err(StageError::stage_ref(
                refname,
                StageRefOp::Update,
                &update_ref.stderr,
            ));
        }

        let push = ctx
            .git()
            .arg("push")
            .arg("origin")
            .arg("--atomic")
            .arg("--porcelain")
            .arg(format!("+{}:{}", refname, refname))
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !push.status.success() {
            return Err(StageError::stage_ref(
                refname,
                StageRefOp::Push,
                &push.stderr,
            ));
        }

        Ok(())
    }

    /// Tag the current `HEAD` of the stage as a named ref.
    fn tag_latest_ref(
        &self,
        reason: &str,
        date_format: &str,
    ) -> StageResult<(DateTime<Utc>, String)> {
        let ctx = self.stager.git_context();
        let now = Utc::now();
        let refname = format!(
            "refs/stage/{}/{}/{}",
            self.branch,
            reason,
            now.format(date_format),
        );

        let update_ref = ctx
            .git()
            .arg("update-ref")
            .arg(&refname)
            .arg(self.stager.head().as_str())
            .arg("0000000000000000000000000000000000000000")
            .output()
            .map_err(|err| GitError::subcommand("update-ref", err))?;
        if !update_ref.status.success() {
            return Err(StageError::stage_ref(
                refname,
                StageRefOp::Tag,
                &update_ref.stderr,
            ));
        }

        let reason_refname = format!("refs/stage/{}/{}/latest", self.branch, reason);

        let update_ref_tagged = ctx
            .git()
            .arg("update-ref")
            .arg(&reason_refname)
            .arg(self.stager.head().as_str())
            .output()
            .map_err(|err| GitError::subcommand("update-ref reason", err))?;
        if !update_ref_tagged.status.success() {
            return Err(StageError::stage_ref(
                refname,
                StageRefOp::Tracking,
                &update_ref_tagged.stderr,
            ));
        }

        let push = ctx
            .git()
            .arg("push")
            .arg("origin")
            .arg("--atomic")
            .arg("--porcelain")
            .arg(format!("+{}:{}", reason_refname, reason_refname))
            .arg(&refname)
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !push.status.success() {
            return Err(StageError::stage_ref(
                refname,
                StageRefOp::PushTag {
                    tracking_ref: reason_refname,
                },
                &push.stderr,
            ));
        }

        Ok((now, refname))
    }

    /// The merge request for a given topic.
    fn hosted_mr(&self, topic: &Topic) -> Result<MergeRequest, HostingServiceError> {
        self.project.merge_request(topic.id)
    }

    /// Update the state of a merge request after being staged.
    fn update_mr_state(
        &self,
        result: &IntegrationResult,
        post_success: bool,
        update_reason: &str,
    ) -> StageResult<()> {
        let mr = self.hosted_mr(result.topic())?;
        match *result {
            IntegrationResult::Staged(_) => {
                self.send_mr_commit_status(&mr, CommitStatusState::Success, "staged");

                if post_success {
                    self.send_info_mr_comment(&mr, "Successfully staged.");
                }
            },
            IntegrationResult::Unstaged(_, ref reason) => {
                self.send_mr_commit_status(
                    &mr,
                    CommitStatusState::Failed,
                    &format!("failed to merge: {}", unstaged_status_desc(reason)),
                );
                self.send_mr_comment(&mr, &unstaged_status_message(reason, update_reason));
            },
            IntegrationResult::Unmerged(_, ref reason) => {
                let (status, desc) = unmerged_status_desc(reason);
                self.send_mr_commit_status(&mr, status, &format!("unstaged: {}", desc));

                let comment_method = if let CommitStatusState::Success = status {
                    Self::send_info_mr_comment
                } else {
                    Self::send_mr_comment
                };

                comment_method(self, &mr, &unmerged_status_message(reason));
            },
        }

        Ok(())
    }

    /// Set the commit status to a merge request.
    fn send_mr_commit_status(&self, mr: &MergeRequest, status: CommitStatusState, desc: &str) {
        let status = mr.create_commit_status(status, "ghostflow-stager", desc);
        if let Err(err) = self.project.service.post_commit_status(status) {
            warn!(
                target: "ghostflow/stage",
                "failed to post a commit status for mr {} on {} for '{}': {:?}",
                mr.id,
                mr.commit.id,
                desc,
                err,
            );
        }
    }

    /// Set the commit status on a commit.
    fn send_commit_status(&self, commit: &Commit, status: CommitStatusState, desc: &str) {
        let status = commit.create_commit_status(status, "ghostflow-stager", desc);
        if let Err(err) = self.project.service.post_commit_status(status) {
            warn!(
                target: "ghostflow/stage",
                "failed to post a commit status on {} for '{}': {:?}",
                commit.id,
                desc,
                err,
            );
        }
    }

    /// Send a comment to a merge request.
    fn send_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if let Err(err) = self.project.service.post_mr_comment(mr, content) {
            error!(
                target: "ghostflow/stage",
                "failed to post a comment to merge request: {}, {}: {:?}",
                self.project.name,
                mr.id,
                err,
            );
        }
    }

    /// Send an informational comment to a merge request.
    fn send_info_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if !self.quiet {
            self.send_mr_comment(mr, content)
        }
    }
}

/// The description for why a merge request has been unstaged.
fn unstaged_status_desc(reason: &UnstageReason) -> String {
    match *reason {
        UnstageReason::MergeConflict(ref conflicts) => {
            format!("{} conflicting paths", conflicts.iter().dedup().count())
        },
    }
}

/// The reason for a merge request update.
fn mr_update_reason(mr: &MergeRequest) -> String {
    format!(
        "an update to the [{}]({}) topic causing ",
        mr.source_branch, mr.url,
    )
}

/// The status message for an unstaged topic.
fn unstaged_status_message(reason: &UnstageReason, update_reason: &str) -> String {
    let reason_message = match *reason {
        UnstageReason::MergeConflict(ref conflicts) => {
            let mut conflict_paths = conflicts
                .iter()
                .map(|conflict| conflict.path().to_string_lossy())
                .dedup();

            format!(
                "merge conflicts in the following paths:\n\n  - `{}`",
                conflict_paths.join("`\n  - `")
            )
        },
    };

    format!(
        "This merge request has been unstaged due to {}{}",
        update_reason, reason_message,
    )
}

/// The description for a merge status.
fn unmerged_status_desc(reason: &MergeStatus) -> (CommitStatusState, &str) {
    match *reason {
        MergeStatus::NoCommonHistory => (CommitStatusState::Failed, "no common history"),
        MergeStatus::AlreadyMerged => (CommitStatusState::Success, "already merged"),
        MergeStatus::Mergeable(_) => {
            error!(
                target: "ghostflow/stage",
                "mergeable unmergeable state?",
            );
            (CommitStatusState::Failed, "mergeable?")
        },
    }
}

/// The status message for a merge status.
fn unmerged_status_message(reason: &MergeStatus) -> String {
    let reason_message = match *reason {
        MergeStatus::NoCommonHistory => "there is no common history",
        MergeStatus::AlreadyMerged => "it has already been merged",
        MergeStatus::Mergeable(_) => "it is\u{2026}mergeable? Sorry, something went wrong",
    };

    format!(
        "This merge request has been unstaged because {}.",
        reason_message,
    )
}
