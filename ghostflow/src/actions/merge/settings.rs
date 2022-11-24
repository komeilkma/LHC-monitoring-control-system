use std::collections::HashMap;
use std::iter;

use chrono::{DateTime, Utc};
use either::{Either, Left, Right};
use git_workarea::{CommitId, GitContext, GitError, Identity, MergeStatus};
use itertools::Itertools;
use log::{debug, error, info, warn};
use topological_sort::TopologicalSort;

use crate::actions::merge::trailers::ParseTrailers;
use crate::actions::merge::{
    InternalMergeError, MergeError, MergePolicy, MergePolicyFilter, MergeResult,
};
use crate::host::{HostedProject, MergeRequest, User};
use crate::utils::Trailer;

/// Information about how to merge into a branch.
#[derive(Debug, Clone)]
pub struct IntoBranch {
    /// The name of the target branch.
    name: String,
    /// Further branches to merge into.
    chain: Vec<IntoBranch>,
}

impl IntoBranch {
    /// Create a new description for an into branch.
    pub fn new<N>(name: N) -> Self
    where
        N: Into<String>,
    {
        Self {
            name: name.into(),
            chain: Vec::new(),
        }
    }

    /// Add branches for further merging.
    pub fn chain_into<I>(&mut self, branch: I) -> &mut Self
    where
        I: IntoIterator<Item = Self>,
    {
        self.chain.extend(branch.into_iter());
        self
    }

    /// The name of the branch.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The branches to chain into.
    pub fn chain_branches(&self) -> &[Self] {
        &self.chain
    }

    pub(crate) fn add_topo_links(&self, sorter: &mut TopologicalSort<String>) {
        self.chain.iter().for_each(|into_branch| {
            debug!("adding dep from {} -> {}", self.name(), into_branch.name());
            sorter.add_dependency(self.name(), into_branch.name());
            into_branch.add_topo_links(sorter);
        })
    }
}

/// The result of the merge action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeActionResult {
    /// Everything worked fine.
    Success,
    /// The push failed.
    ///
    /// This likely means that the remote changed in some way and the merge will need to be
    /// restarted.
    PushFailed,
    /// The merge failed due to conflicts or otherwise unsuitable state of the merge request.
    ///
    /// Failures require user interaction before they may be attempted again.
    Failed,
}

/// The supported merge topologies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeTopology {
    /// Always create a merge commit.
    ///
    /// This merge topology keeps the first-parent history of the branch as a sequence of merges of
    /// topic branches.
    NoFastForward,
    /// Create a merge commit if necessary, but fast forward if possible.
    ///
    /// Note that the actual behavior for this topology depends on the timing and series of
    /// actions. A topic branch might be eligible for fast-forward, but create a merge commit
    /// later.
    FastForwardIfPossible,
    /// Never create a merge commit.
    ///
    /// The topic branch will be pushed directly to the target branch. If it cannot be pushed, it
    /// will result in a failure to merge.
    FastForwardOnly,
}

impl MergeTopology {
    fn allow_fast_forward(self) -> bool {
        !matches!(self, Self::NoFastForward)
    }

    fn must_fast_forward(self) -> bool {
        matches!(self, Self::FastForwardOnly)
    }
}

/// Settings for a merge action.
#[derive(Debug, Clone)]
pub struct MergeSettings<P> {
    /// The branch that the merge action is responsible for targetting.
    branch: String,
    /// Branch name for merging.
    merge_branch_as: Option<String>,
    /// Branches that should always contain the target branch.
    into_branches: Vec<IntoBranch>,
    /// The merge policy.
    policy: P,
    /// Whether the action should create informational comments or not.
    ///
    /// Errors always create comments.
    quiet: bool,
    /// The number of commits to list in the merge commit message.
    ///
    /// A limit of `None` lists all commits in the message.
    log_limit: Option<usize>,
    /// Whether to elide this branch's name in merge commits or not.
    ///
    /// When elided, the `into <branch>` does not appear in the merge commit summaries for this
    /// branch.
    elide_branch_name: bool,
    /// The merge topology to use.
    ///
    /// This controls how the topic is merged into the target branch.
    merge_topology: MergeTopology,
}

impl<P> MergeSettings<P> {
    /// Create a new merge action.
    pub fn new<B>(branch: B, policy: P) -> Self
    where
        B: Into<String>,
    {
        Self {
            branch: branch.into(),
            merge_branch_as: None,
            into_branches: Vec::new(),
            policy,
            quiet: false,
            log_limit: None,
            elide_branch_name: false,
            merge_topology: MergeTopology::NoFastForward,
        }
    }

    /// Reduce the number of comments made by the merge action.
    ///
    /// The comments created by this action can be a bit much. This reduces the comments to those
    /// which are errors or are important.
    pub fn quiet(&mut self) -> &mut Self {
        self.quiet = true;
        self
    }

    /// Whether to merge silently or not.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Limit the number of log entries in merge commit messages.
    ///
    /// Anything beyond this limit (if present) is elided.
    pub fn log_limit(&mut self, log_limit: Option<usize>) -> &mut Self {
        self.log_limit = log_limit;
        self
    }

    /// Set the name of the branch to use in merge commits.
    pub fn merge_branch_as<N>(&mut self, name: Option<N>) -> &mut Self
    where
        N: Into<String>,
    {
        self.merge_branch_as = name.map(|name| name.into());
        self
    }

    /// Whether to elide the target branch name when merging.
    pub fn elide_branch_name(&mut self, elide: bool) -> &mut Self {
        self.elide_branch_name = elide;
        self
    }

    /// Set the merge topology.
    pub fn merge_topology(&mut self, topology: MergeTopology) -> &mut Self {
        self.merge_topology = topology;
        self
    }

    /// The name of the branch to use in merge commits.
    pub fn merge_name(&self) -> (bool, &str) {
        (
            self.elide_branch_name,
            self.merge_branch_as.as_ref().unwrap_or(&self.branch),
        )
    }

    /// Add a set of branches which always have this branch merged in.
    pub fn add_into_branches<I>(&mut self, branches: I) -> &mut Self
    where
        I: IntoIterator<Item = IntoBranch>,
    {
        self.into_branches.extend(branches.into_iter());
        self
    }

    /// The name of the branch being merged into.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Branches which should be merged into when a topic is merged into this branch.
    #[allow(clippy::wrong_self_convention)]
    pub fn into_branches(&self) -> &[IntoBranch] {
        &self.into_branches
    }

    fn add_topo_links(&self, sorter: &mut TopologicalSort<String>) {
        self.into_branches.iter().for_each(|into_branch| {
            debug!(
                "adding dep from {} -> {}",
                self.branch(),
                into_branch.name(),
            );
            sorter.add_dependency(self.branch(), into_branch.name());
            into_branch.add_topo_links(sorter);
        })
    }
}

/// Information required when performing a merge.
pub struct MergeInformation<'a> {
    /// The name of the topic that is being merged.
    pub topic_name: &'a str,
    /// Who is performing the merge.
    pub who: &'a Identity,
    /// When the merge was requested.
    pub when: DateTime<Utc>,
}

/// A structure which performs a merge of a merge request.
pub struct Merger<'a> {
    /// The git context to use for merging.
    ctx: &'a GitContext,
    /// The project that is being merged into.
    project: &'a HostedProject,
    /// The merge request which is being merged.
    mr: &'a MergeRequest,
    /// The trailers discovered from the merge request.
    trailers: Vec<(Trailer, Option<User>)>,
}

/// Alias used for nested results involved in merging a merge request.
type StepResult<T> = MergeResult<Either<T, MergeActionResult>>;

impl<'a> Merger<'a> {
    /// Create a new merger object.
    pub fn new(
        ctx: &'a GitContext,
        project: &'a HostedProject,
        mr: &'a MergeRequest,
    ) -> MergeResult<Self> {
        Ok(Merger {
            ctx,
            project,
            mr,
            trailers: ParseTrailers::find(project, mr)?,
        })
    }

    /// Merge a merge request.
    pub fn merge_mr<P>(
        self,
        settings: &MergeSettings<P>,
        info: MergeInformation<'_>,
    ) -> MergeResult<MergeActionResult>
    where
        P: MergePolicy,
    {
        if let Right(res) = self.prep_mr()? {
            return Ok(res);
        }

        let commit_id = match self.create_merge(settings, &info, &self.mr.commit.id)? {
            Left(commit_id) => commit_id,
            Right(res) => {
                return Ok(res);
            },
        };

        let mut sorter = TopologicalSort::new();
        let mut renamer = HashMap::new();
        renamer.insert(settings.branch.clone(), settings.merge_name());
        settings.add_topo_links(&mut sorter);
        let refs = iter::once((settings.branch.clone(), commit_id, settings.into_branches()));
        let push_refs = self.perform_update_merges(sorter, refs, &info, renamer)?;
        self.push_refs(settings.quiet, push_refs)
    }

    /// Prepare to merge a merge request.
    ///
    /// This ensures the merge request is available locally and that it is not a work-in-progress.
    pub fn prep_mr(&self) -> StepResult<()> {
        info!(
            target: "ghostflow/merge",
            "preparing to merge {}",
            self.mr.url,
        );

        if self.mr.work_in_progress {
            self.send_mr_comment(
                "This merge request is marked as a Work in Progress and may not be merged. Please \
                 remove the Work in Progress state first.",
            );
            return Ok(Right(MergeActionResult::Failed));
        }

        // Fetch the commit into the merge's git context.
        self.project.service.fetch_mr(self.ctx, self.mr)?;

        Ok(Left(()))
    }

    /// Perform merges from updated branches into their "into" branches.
    ///
    /// This takes the sorted set of branches which have been merged into by a merge request and
    /// updates all of their "into" branches so that they are always synchronized.
    ///
    /// Returns a vector of commits which need to be pushed to the given branches on the remote.
    pub fn perform_update_merges<'b, I>(
        &self,
        mut sorter: TopologicalSort<String>,
        refs: I,
        info: &MergeInformation<'b>,
        renamer: HashMap<String, (bool, &str)>,
    ) -> MergeResult<Vec<(CommitId, String)>>
    where
        I: IntoIterator<Item = (String, CommitId, &'b [IntoBranch])>,
    {
        // A map of branch -> Vec<branch> for knowing which branches to merge into this branch.
        let mut from_branches: HashMap<_, Vec<_>> = HashMap::new();
        // A map of branch -> (commit, [IntoBranch]) to know where branches should go.
        let mut push_refs = refs
            .into_iter()
            .map(|(branch, commit, into_branches)| (branch, (commit, into_branches)))
            .collect::<HashMap<_, _>>();

        // Look at a free branch.
        while let Some(target_branch) = sorter.pop() {
            let (target_commit, target_intos) =
                if let Some(target_info) = push_refs.get(&target_branch) {
                    // Merge into the branch as it has been updated so far.
                    target_info.clone()
                } else {
                    // There is no record of what commit is supposed to be used for this ref, but
                    // the end of this loop should always be keeping this up-to-date.
                    return Err(InternalMergeError::reference_tracking(target_branch).into());
                };

            // Find out if we have branches to merge into the branch.
            if let Some(source_branches) = from_branches.remove(target_branch.as_str()) {
                // Perform the merges.
                let new_commit = source_branches.into_iter().fold(
                    Ok(target_commit) as MergeResult<_>,
                    |target_commit, source_branch: String| {
                        let target_commit = target_commit?;
                        let &(ref source_commit, _) =
                            push_refs.get(&source_branch).ok_or_else(|| {
                                InternalMergeError::into_branch_sorting(
                                    source_branch.clone(),
                                    target_branch.clone(),
                                )
                            })?;

                        // Prepare for the merge of the branch.
                        let workarea = self.ctx.prepare(&target_commit)?;
                        let merge_result =
                            workarea.setup_update_merge(&target_commit, source_commit)?;
                        let mut merge_command =
                            if let git_workarea::MergeResult::Ready(command) = merge_result {
                                command
                            } else {
                                // Unmergeable; something went really wrong since we're not
                                // actually performing a merge, but just creating a new commit with
                                // the same tree and multiple parents.
                                return Err(InternalMergeError::merge_ours(merge_result).into());
                            };

                        // Add authorship information to the commit message. Committer information is provided by
                        // the default git environment.
                        merge_command.author(info.who).author_date(info.when);

                        // Get the merge-as name for the branch.
                        let into_branch = match renamer
                            .get(&target_branch)
                            .unwrap_or(&(false, &target_branch))
                        {
                            (true, _) => String::new(),
                            (false, name) => format!(" into {}", name),
                        };

                        let source_branch_name = renamer
                            .get(&source_branch)
                            .map_or(source_branch.as_ref(), |(_, name)| *name);

                        let commit_message =
                            format!("Merge branch '{}'{}", source_branch_name, into_branch);

                        info!(
                            target: "ghostflow/merge",
                            "updating {} into {}",
                            source_branch,
                            target_branch,
                        );

                        // Fold using the newly created merge commit.
                        Ok(merge_command.commit(commit_message)?)
                    },
                )?;

                let push_ref = push_refs
                    .entry(target_branch.clone())
                    .or_insert_with(|| (CommitId::new(String::new()), target_intos));
                push_ref.0 = new_commit;
            }

            // Queue up the "into" branches for this branch.
            target_intos.iter().for_each(|target_into| {
                from_branches
                    .entry(target_into.name())
                    .or_insert_with(Vec::new)
                    .push(target_branch.clone());
                push_refs.entry(target_into.name().into()).or_insert((
                    CommitId::new(target_into.name()),
                    target_into.chain_branches(),
                ));
            });
        }

        // If there are still nodes remaining, we have detected a cycle.
        if !sorter.is_empty() {
            return Err(MergeError::circular_into_branches());
        }

        // If we still have information about branches here, we haven't merged everything required.
        // This should never happen (we'd have had circular dependencies).
        if !from_branches.is_empty() {
            return Err(
                InternalMergeError::leftover_branches(from_branches.keys().copied()).into(),
            );
        }

        Ok(push_refs
            .into_iter()
            .map(|(branch, (commit, _))| (commit, branch))
            .collect())
    }

    /// Create a merge commit for the merge request into the branch.
    pub fn create_merge<'b, P>(
        &self,
        settings: &MergeSettings<P>,
        info: &MergeInformation<'b>,
        commit_id: &CommitId,
    ) -> StepResult<CommitId>
    where
        P: MergePolicy,
    {
        info!(
            target: "ghostflow/merge",
            "preparing to merge {} into {}",
            self.mr.url,
            settings.branch,
        );

        let branch_id = CommitId::new(&settings.branch);

        // Determine if the topic is mergeable at all.
        let merge_status = self.ctx.mergeable(&branch_id, commit_id)?;
        let bases = if let MergeStatus::Mergeable(bases) = merge_status {
            bases
        } else {
            self.send_mr_comment(&unmerged_status_message(&settings.branch, &merge_status));
            return Ok(Right(MergeActionResult::Failed));
        };

        // Check the desired topology and the potential.
        if settings.merge_topology.allow_fast_forward() {
            let can_fast_forward = self
                .ctx
                .git()
                .arg("merge-base")
                .arg("--is-ancestor")
                .arg(&settings.branch)
                .arg(commit_id.as_str())
                .status()
                .map_err(|err| GitError::subcommand("merge-base --is-ancestor", err))?;

            if can_fast_forward.success() {
                // Use the to-be-merged commit as the merge commit.
                return Ok(Left(commit_id.clone()));
            } else if settings.merge_topology.must_fast_forward() {
                // A fast-forward merge is not possible.
                self.send_mr_comment(&no_fast_forward_possible_message(&settings.branch));
                return Ok(Right(MergeActionResult::Failed));
            }
        }

        // Prepare a work area to perform the actual merge.
        let workarea = self.ctx.prepare(&branch_id)?;
        let merge_result = workarea.setup_merge(&bases, &branch_id, commit_id)?;
        let mut merge_command = match merge_result {
            git_workarea::MergeResult::Conflict(conflicts) => {
                let mut conflict_paths = conflicts
                    .iter()
                    .map(|conflict| conflict.path().to_string_lossy())
                    .dedup();
                self.send_mr_comment(&format!(
                    "This merge request contains conflicts with `{}` in \
                     the following paths:\n\n  - `{}`",
                    settings.branch,
                    conflict_paths.join("`\n  - `"),
                ));
                return Ok(Right(MergeActionResult::Failed));
            },
            git_workarea::MergeResult::Ready(command) => command,
        };

        // Add authorship information to the commit message. Committer information is provided by
        // the default git environment.
        merge_command.author(info.who).author_date(info.when);

        let mut mr_policy = settings.policy.for_mr(self.mr);

        self.trailers
            .iter()
            // Filter trailers through the policy.
            .for_each(|&(ref trailer, ref user_opt)| {
                mr_policy.process_trailer(trailer, user_opt.as_ref())
            });

        let trailers = match mr_policy.result() {
            Ok(trailers) => trailers.into_iter().unique(),
            Err(reasons) => {
                let reason = reasons.into_iter().join("  \n  - ");
                self.send_mr_comment(&format!(
                    "This merge request may not be merged into `{}` \
                     because:\n\n  \
                     - {}",
                    settings.branch, reason,
                ));
                return Ok(Right(MergeActionResult::Failed));
            },
        };

        let commit_message =
            self.build_commit_message(settings, info.topic_name, commit_id, trailers)?;

        info!(
            target: "ghostflow/merge",
            "merging {} into {}",
            self.mr.url,
            settings.branch,
        );

        Ok(Left(merge_command.commit(commit_message)?))
    }

    /// Push the results of a merge action to the remote repository.
    pub fn push_refs<R, B>(&self, quiet: bool, refs: R) -> MergeResult<MergeActionResult>
    where
        R: IntoIterator<Item = (CommitId, B)>,
        B: AsRef<str>,
    {
        let push = self
            .ctx
            .git()
            .arg("push")
            .arg("--atomic")
            .arg("--porcelain")
            .arg("origin")
            .args(
                &refs
                    .into_iter()
                    .map(|(commit_id, branch)| format!("{}:{}", commit_id, branch.as_ref()))
                    .collect::<Vec<_>>(),
            )
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !push.status.success() {
            warn!(
                target: "ghostflow/merge",
                "failed to push the merge of {} to the remote server: {}",
                self.mr.url,
                String::from_utf8_lossy(&push.stderr),
            );

            self.send_info_mr_comment(
                quiet,
                "Automatic merge succeeded, but pushing to the remote failed.",
            );

            return Ok(MergeActionResult::PushFailed);
        }

        self.send_info_mr_comment(quiet, "Topic successfully merged and pushed.");

        Ok(MergeActionResult::Success)
    }

    /// Build a commit message for a merge request.
    fn build_commit_message<I, P>(
        &self,
        settings: &MergeSettings<P>,
        topic_name: &str,
        commit_id: &CommitId,
        trailers: I,
    ) -> MergeResult<String>
    where
        I: IntoIterator<Item = Trailer>,
        P: MergePolicy,
    {
        let mut topic_summary = self
            .mr
            .description
            // Break it into lines.
            .lines()
            // Find the message block.
            .skip_while(|&line| line != "```message")
            // Skip the entry line.
            .skip(1)
            // Take it until the end of the block.
            .take_while(|&line| line != "```")
            // Add newlines.
            .map(|line| format!("{}\n", line))
            // Join the lines together.
            .join("");
        if !topic_summary.is_empty() {
            // Append a separator if we have a topic description.
            topic_summary.push('\n');
        }

        let mut log_command = self.ctx.git();
        log_command
            .arg("log")
            .arg("--date-order")
            .arg("--format=%h %s")
            .arg("--abbrev-commit");

        if let Some(limit) = settings.log_limit {
            // Get up to one more than the maximum. This is done so that we can detect that there
            // are more so that an elision indicator may be added.
            log_command.arg(format!("--max-count={}", limit + 1));
        }

        let log = log_command
            .arg(format!("{}..{}", settings.branch, commit_id))
            .output()
            .map_err(|err| GitError::subcommand("log", err))?;
        if !log.status.success() {
            return Err(MergeError::log_topic(&log.stderr));
        }
        let log_output = String::from_utf8_lossy(&log.stdout);
        let mut log_lines = log_output.lines().collect::<Vec<_>>();
        // Elide the log if there are too many entries.
        if let Some(limit) = settings.log_limit {
            if limit == 0 {
                log_lines.clear()
            } else if log_lines.len() > limit {
                log_lines[limit] = "...";
            }
        }
        let mut log_summary = log_lines
            .into_iter()
            .map(|line| format!("{}\n", line))
            .join("");
        if !log_summary.is_empty() {
            // Append a separator if we have a log summary.
            log_summary.push('\n');
        }

        let trailer_summary = trailers
            .into_iter()
            .chain(iter::once(Trailer::new(
                "Merge-request",
                &self.mr.reference,
            )))
            .map(|trailer| format!("{}\n", trailer))
            .join("");

        let into_branch = match settings.merge_name() {
            (true, _) => String::new(),
            (false, name) => format!(" into {}", name),
        };

        Ok(format!(
            "Merge topic '{}'{}\n\
             \n\
             {}{}{}",
            topic_name, into_branch, topic_summary, log_summary, trailer_summary,
        ))
    }

    /// Send a comment to a merge request.
    fn send_mr_comment(&self, content: &str) {
        if let Err(err) = self.project.service.post_mr_comment(self.mr, content) {
            error!(
                target: "ghostflow/merge",
                "failed to post a comment to merge request: {}, {}: {:?}",
                self.project.name,
                self.mr.id,
                err,
            );
        }
    }

    /// Send an informational comment to a merge request.
    fn send_info_mr_comment(&self, quiet: bool, content: &str) {
        if !quiet {
            self.send_mr_comment(content)
        }
    }
}

/// The status message for a merge status.
fn unmerged_status_message(branch: &str, reason: &MergeStatus) -> String {
    let reason_message = match *reason {
        MergeStatus::NoCommonHistory => "there is no common history",
        MergeStatus::AlreadyMerged => "it has already been merged",
        MergeStatus::Mergeable(_) => "it is\u{2026}mergeable? Sorry, something went wrong",
    };

    format!(
        "This merge request may not be merged into `{}` because {}.",
        branch, reason_message,
    )
}

/// The comment for a failed fast forward merge.
fn no_fast_forward_possible_message(branch: &str) -> String {
    format!(
        "This merge request may not be merged into `{}` because a fast-forward merge was \
         requested, but is not possible because the branch is no longer an ancestor.",
        branch,
    )
}
