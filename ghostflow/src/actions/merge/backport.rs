
//! Merging with backporting support.
//!
//! This action may be used when a merge request targets a branch, but also contains pieces which
//! need to be backported to other branches. It performs all of the relevant merges and pushes them
//! to the repository at once.

use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

use chrono::{DateTime, Utc};
use either::{Left, Right};
use git_workarea::{CommitId, GitContext, Identity};
use log::debug;
use topological_sort::TopologicalSort;

use crate::actions::merge::prelude_impl::*;
use crate::host::{HostedProject, MergeRequest};
use crate::utils::mr::{self, CommitMergeRequestState};

/// A backport settings structure with the commit that should be merged.
pub struct MergeBackport<'a, P>
where
    P: 'a,
{
    /// The settings to use for merging.
    settings: &'a MergeSettings<P>,
    /// The commit to use from the topic.
    commit: Option<CommitId>,
}

impl<'a, P> MergeBackport<'a, P>
where
    P: 'a,
{
    /// Create a new backport description.
    pub fn new(settings: &'a MergeSettings<P>, commit: Option<CommitId>) -> Self {
        MergeBackport {
            settings,
            commit,
        }
    }
}

/// Merge a merge request into multiple target branches.
pub struct MergeMany {
    /// The context to use for Git actions.
    ctx: GitContext,
    /// The project of the target branches.
    project: HostedProject,
}

impl MergeMany {
    /// Create a new merge action.
    pub fn new(ctx: GitContext, project: HostedProject) -> Self {
        Self {
            ctx,
            project,
        }
    }

    /// Merge a merge request into multiple target branches.
    ///
    /// Information for the merge commit is gathered from the comment stream as well as the merge
    /// request itself. Comments from before the last update are ignored since they do not apply to
    /// the latest incarnation of the topic.
    pub fn merge_mr<'a, I, P>(
        &self,
        mr: &MergeRequest,
        who: &Identity,
        when: DateTime<Utc>,
        with: I,
    ) -> MergeResult<MergeActionResult>
    where
        I: IntoIterator<Item = MergeBackport<'a, P>>,
        P: MergePolicy + 'a,
    {
        self.merge_mr_named(mr, &mr.source_branch, who, when, with)
    }

    /// Merge a merge request into the branch with a different name.
    pub fn merge_mr_named<'a, T, I, P>(
        &self,
        mr: &MergeRequest,
        topic_name: T,
        who: &Identity,
        when: DateTime<Utc>,
        with: I,
    ) -> MergeResult<MergeActionResult>
    where
        T: AsRef<str>,
        I: IntoIterator<Item = MergeBackport<'a, P>>,
        P: MergePolicy + 'a,
    {
        let merger = Merger::new(&self.ctx, &self.project, mr)?;
        let info = MergeInformation {
            topic_name: topic_name.as_ref(),
            who,
            when,
        };

        if let Right(res) = merger.prep_mr()? {
            return Ok(res);
        }

        let mut refs = HashMap::new();
        let mut target_branches = HashSet::new();
        let mut sorter = TopologicalSort::new();
        let mut renamer = HashMap::new();

        for backport in with {
            let branch: &str = backport.settings.branch();
            let commit = {
                if !target_branches.insert(backport.settings.branch()) {
                    return Err(MergeError::duplicate_target_branch(
                        backport.settings.branch().into(),
                    ));
                }

                let commit_id = backport.commit.as_ref().unwrap_or(&mr.commit.id);

                // Ensure that the commit is part of the merge request.
                let commit_state = mr::commit_state(
                    &self.ctx,
                    mr,
                    commit_id,
                    &CommitId::new(backport.settings.branch()),
                )?;
                match commit_state {
                    CommitMergeRequestState::OnMergeRequest => (),
                    CommitMergeRequestState::OnTarget => {
                        return Err(MergeError::merged_commit(commit_id.clone()));
                    },
                    CommitMergeRequestState::Unrelated => {
                        // Explicitly do not contact the hosting service here; the caller should collect
                        // this information and handle it.
                        return Err(MergeError::unrelated_commit(commit_id.clone()));
                    },
                }

                let merge_res = merger.create_merge(backport.settings, &info, commit_id)?;
                match merge_res {
                    Left(commit_id) => commit_id,
                    Right(res) => {
                        return Ok(res);
                    },
                }
            };

            backport
                .settings
                .into_branches()
                .iter()
                .for_each(|into_branch| {
                    debug!("adding dep from {} -> {}", branch, into_branch.name());
                    sorter.add_dependency(branch, into_branch.name());
                    into_branch.add_topo_links(&mut sorter)
                });

            renamer.insert(branch.into(), backport.settings.merge_name());
            refs.insert(branch, (commit, backport.settings));
        }

        let quiet = refs.iter().all(|(_, &(_, settings))| settings.is_quiet());
        let refs_status = refs.iter().map(|(&branch, &(ref commit, settings))| {
            (branch.into(), commit.clone(), settings.into_branches())
        });
        let push_refs = merger.perform_update_merges(sorter, refs_status, &info, renamer)?;

        merger.push_refs(quiet, push_refs)
    }
}
