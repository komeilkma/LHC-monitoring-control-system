//! Simple merging.
//!
//! This action may be used when a merge request targets a single branch.

use chrono::{DateTime, Utc};
use git_workarea::{GitContext, Identity};

use crate::actions::merge::prelude_impl::*;
use crate::host::{HostedProject, MergeRequest};

/// Implementation of the `merge` action.
#[derive(Debug)]
pub struct Merge<P> {
    /// The settings for the merge action.
    settings: MergeSettings<P>,
    /// The context to use for Git actions.
    ctx: GitContext,
    /// The project of the target branch.
    project: HostedProject,
}

impl<P> Merge<P> {
    /// Create a new merge action.
    pub fn new(ctx: GitContext, project: HostedProject, settings: MergeSettings<P>) -> Self {
        Self {
            settings,
            ctx,
            project,
        }
    }

    /// The settings for the merge action.
    pub fn settings(&self) -> &MergeSettings<P> {
        &self.settings
    }
}

impl<P> Merge<P>
where
    P: MergePolicy,
{
    /// Merge a merge request into the branch.
    ///
    /// Information for the merge commit is gathered from the comment stream as well as the merge
    /// request itself. Comments from before the last update are ignored since they do not apply to
    /// the latest incarnation of the topic.
    pub fn merge_mr(
        &self,
        mr: &MergeRequest,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> MergeResult<MergeActionResult> {
        self.merge_mr_named(mr, &mr.source_branch, who, when)
    }

    /// Merge a merge request into the branch with a different name.
    pub fn merge_mr_named<B>(
        &self,
        mr: &MergeRequest,
        topic_name: B,
        who: &Identity,
        when: DateTime<Utc>,
    ) -> MergeResult<MergeActionResult>
    where
        B: AsRef<str>,
    {
        let merger = Merger::new(&self.ctx, &self.project, mr)?;
        let info = MergeInformation {
            topic_name: topic_name.as_ref(),
            who,
            when,
        };
        merger.merge_mr(&self.settings, info)
    }
}
