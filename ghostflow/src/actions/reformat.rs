
//! The `reformat` action.
//!
//! This action takes a merge request and reformats all commits it adds to its target branch and
//! reformats each commit using tools to format selected files within the repository according to
//! the appropriate `format.{kind}` attribute on files changed in a commit.
//!
//! There may be multiple formatters to run on a given repository. Each is given a `kind` (used in
//! attribute lookups), a path to an executable to run to perform the formatting, and a set of
//! files within the repository which contain configuration files for the formatter.

use std::collections::hash_map::HashMap;
use std::fmt::{self, Write as _};
use std::io::{self, Read, Write};
use std::iter;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use git_checks_core::{AttributeState, CheckGitContext, Commit, Content, FileName};
use git_workarea::{CommitId, GitContext, GitError, GitWorkArea, WorkAreaError};
use itertools::Itertools;
use log::{error, info, warn};
use rayon::prelude::*;
use thiserror::Error;
use wait_timeout::ChildExt;

use crate::host::{HostedProject, MergeRequest};

/// The stage of the format execution.
#[derive(Debug, Clone, Copy)]
pub enum FormatExecStage {
    /// Running the formatter.
    Run,
    /// Waiting for the formatter to complete.
    Wait,
    /// Killing a timed-out formatter.
    Kill,
    /// Waiting for a to-be-killed formatter to exit.
    ///
    /// If this happens, a formatter is left in a zombie state.
    TimeoutWait,
}

impl fmt::Display for FormatExecStage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let what = match self {
            FormatExecStage::Run => "execute",
            FormatExecStage::Wait => "wait on",
            FormatExecStage::Kill => " kill (timed out)",
            FormatExecStage::TimeoutWait => " wait on (timed out)",
        };

        write!(f, "{}", what)
    }
}

/// Errors which may occur when running a formatter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FormatterError {
    /// An empty kind was given.
    #[error("an empty `kind` was given")]
    EmptyKind {},
    /// A formatter path does not exist.
    #[error("the formatter path does not exist: {}", formatter.display())]
    MissingFormatter {
        /// The path to the formatter.
        formatter: PathBuf,
    },
    /// Failure to execute a formatter.
    #[error("failed to {} the {} formatter: {}", stage, command.display(), source)]
    ExecFormatter {
        /// The formatter command.
        command: PathBuf,
        /// The stage of execution.
        stage: FormatExecStage,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to collect stderr from a failed formatter.
    #[error("failed to collect stderr from the {} formatter: {}", command.display(), source)]
    CollectStderr {
        /// The formatter command.
        command: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to check an attribute of a file.
    #[error("git attribute error: {}", source)]
    Attribute {
        /// The source of the error.
        #[from]
        source: git_checks_core::AttributeError,
    },
}

impl FormatterError {
    fn empty_kind() -> Self {
        FormatterError::EmptyKind {}
    }

    fn missing_formatter(formatter: PathBuf) -> Self {
        FormatterError::MissingFormatter {
            formatter,
        }
    }

    fn exec_formatter(command: PathBuf, stage: FormatExecStage, source: io::Error) -> Self {
        FormatterError::ExecFormatter {
            command,
            stage,
            source,
        }
    }

    fn collect_stderr(command: PathBuf, source: io::Error) -> Self {
        FormatterError::CollectStderr {
            command,
            source,
        }
    }
}

type FormatterResult<T> = Result<T, FormatterError>;

/// A formatter for source code in a repository.
///
/// The formatter is passed the file it is expected to format as its only argument.
///
/// Generally, formatters should be idempotent so that lines are not changed multiple times
/// over the course of a topic.
///
/// Untracked files left at the end of the formatter are treated as a failure.
#[derive(Debug)]
pub struct Formatter {
    /// The `kind` of the formatter (used for attribute queries).
    kind: String,
    /// The path to the formatter.
    formatter: PathBuf,
    /// Configuration files within the repository for the formatter.
    config_files: Vec<String>,
    /// A timeout for running the formatter.
    ///
    /// If the formatter exceeds this timeout, it is considered to have failed.
    timeout: Option<Duration>,
}

/// How long to wait for a timed-out formatter to respond to `SIGKILL` before leaving it as a
/// zombie process.
const ZOMBIE_TIMEOUT: Duration = Duration::from_secs(1);

impl Formatter {
    /// Create a new formatter.
    pub fn new<K, F>(kind: K, formatter: F) -> FormatterResult<Self>
    where
        K: Into<String>,
        F: AsRef<Path>,
    {
        let kind = kind.into();
        if kind.is_empty() {
            return Err(FormatterError::empty_kind());
        }

        let formatter = formatter.as_ref().to_path_buf();
        if !formatter.exists() {
            return Err(FormatterError::missing_formatter(formatter));
        }

        Ok(Self {
            kind,
            formatter,
            config_files: Vec::new(),
            timeout: None,
        })
    }

    /// Add configuration files within the repository which should be checked out.
    pub fn add_config_files<I, F>(&mut self, files: I) -> &mut Self
    where
        I: IntoIterator<Item = F>,
        F: Into<String>,
    {
        self.config_files.extend(files.into_iter().map(Into::into));
        self
    }

    /// Add a timeout to the formatter.
    pub fn with_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = Some(timeout);
        self
    }

    /// Format a path within the repository.
    fn format_path<'a>(
        &self,
        workarea: &GitWorkArea,
        path: &'a FileName,
        attr_value: Option<String>,
    ) -> FormatterResult<Option<&'a FileName>> {
        let mut cmd = Command::new(&self.formatter);
        workarea.cd_to_work_tree(&mut cmd);
        cmd.arg(path.as_path());
        if let Some(attr_value) = attr_value {
            cmd.arg(attr_value);
        }

        let (success, output) = if let Some(timeout) = self.timeout {
            let mut child = cmd
                // Formatters should not read anything.
                .stdin(Stdio::null())
                // The output goes nowhere.
                .stdout(Stdio::null())
                // But we want any error messages from them (for logging purposes). If this pipe
                // fills up buffers, it will deadlock and the timeout will "save" us. Any process
                // outputting this much error messages probably is very unhappy anyways.
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| {
                    FormatterError::exec_formatter(
                        self.formatter.clone(),
                        FormatExecStage::Run,
                        err,
                    )
                })?;
            let check = child.wait_timeout(timeout).map_err(|err| {
                FormatterError::exec_formatter(self.formatter.clone(), FormatExecStage::Wait, err)
            })?;

            if let Some(status) = check {
                let stderr = child.stderr.expect("spawned with stderr");
                let bytes_output = stderr
                    .bytes()
                    .collect::<Result<Vec<u8>, _>>()
                    .map_err(|err| FormatterError::collect_stderr(self.formatter.clone(), err))?;
                (
                    status.success(),
                    format!(
                        "failed with exit code {:?}, signal {:?}, output: {:?}",
                        status.code(),
                        status.signal(),
                        String::from_utf8_lossy(&bytes_output),
                    ),
                )
            } else {
                child.kill().map_err(|err| {
                    FormatterError::exec_formatter(
                        self.formatter.clone(),
                        FormatExecStage::Kill,
                        err,
                    )
                })?;
                let timed_out_status = child.wait_timeout(ZOMBIE_TIMEOUT).map_err(|err| {
                    FormatterError::exec_formatter(
                        self.formatter.clone(),
                        FormatExecStage::TimeoutWait,
                        err,
                    )
                })?;
                if timed_out_status.is_none() {
                    warn!(
                        target: "ghostflow/formatting",
                        "leaving a zombie '{}' process; it did not respond to kill",
                        self.kind,
                    );
                }
                (false, "timeout reached".into())
            }
        } else {
            let check = cmd.output().map_err(|err| {
                FormatterError::exec_formatter(self.formatter.clone(), FormatExecStage::Run, err)
            })?;
            (
                check.status.success(),
                String::from_utf8_lossy(&check.stderr).into_owned(),
            )
        };

        Ok(if success {
            None
        } else {
            info!(
                target: "ghostflow/reformat",
                "failed to run the {} formatter: {}",
                self.kind,
                output,
            );
            Some(path)
        })
    }
}

/// Reasons for listing files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFilesReason {
    /// Files a formatter deleted.
    Deleted,
    /// Files a formatter created.
    Untracked,
}

impl fmt::Display for ListFilesReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let what = match self {
            ListFilesReason::Deleted => "deleted",
            ListFilesReason::Untracked => "untracked",
        };

        write!(f, "{}", what)
    }
}

/// Reasons a file is disallowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisallowedFilesReason {
    /// Deleting files is not a valid reformat.
    Deleted,
    /// Creating unrelated files is not a valid reformat.
    Created,
}

impl fmt::Display for DisallowedFilesReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let what = match self {
            DisallowedFilesReason::Deleted => "deleted",
            DisallowedFilesReason::Created => "created",
        };

        write!(f, "{}", what)
    }
}

/// Errors which may occur when reformatting a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReformatError {
    /// A merge commit may not be used to reformat the entire repository.
    #[error("merge commits cannot be used to reformat repositories")]
    MergeCommit {},
    /// The repository's files could not be listed.
    #[error("failed to list all files: {}", output)]
    ListAllFiles {
        /// Output from `git ls-files`.
        output: String,
    },
    /// Git gave us a bad format for a file.
    #[error("failed to parse a line from `ls-tree -r`: {}", line)]
    ListAllFilesFormat {
        /// The bad line.
        line: String,
    },
    /// Commits on a merge request could not be listed.
    #[error("failed to list merge request commits: {}", output)]
    ListCommits {
        /// Output from `git rev-list`.
        output: String,
    },
    /// Reformatted files could not be added to the index.
    #[error("failed to add reformatted files: {}", output)]
    AddFiles {
        /// Output from `git add`.
        output: String,
    },
    /// The reformatted tree could not be written.
    #[error("failed to write the reformatted tree: {}", output)]
    WriteTree {
        /// Output from `git write-tree`.
        output: String,
    },
    /// The reformatted tree could not be written.
    #[error("failed to write the commit message to commit-tree: {}", source)]
    WriteCommitMessage {
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// The reformatted tree could not be committed.
    #[error("failed to commit the reformatted tree: {}", output)]
    CommitTree {
        /// Output from `git commit-tree`.
        output: String,
    },
    /// The tree for a commit could not be found.
    #[error("failed to get the tree for {}: {}", commit, output)]
    FindCommitTree {
        /// The commit.
        commit: CommitId,
        /// Output from `git rev-parse`.
        output: String,
    },
    /// The tree for a commit could not be found.
    #[error("failed to get the dates for {}: {}", commit, output)]
    FindCommitDates {
        /// The commit.
        commit: CommitId,
        /// The output of `git log`.
        output: String,
    },
    /// Parsing an author or committer date from a commit failed.
    #[error("parsing a date: {}", source)]
    DateParse {
        /// The source of the error.
        #[from]
        source: chrono::ParseError,
    },
    /// A formatter failed.
    #[error("failed to reformat commit {} files: `{}`", commit, paths.iter().format("`, `"))]
    ReformatFailed {
        /// The commit that could not be reformatted.
        commit: CommitId,
        /// The paths that could not be reformatted.
        paths: Vec<String>,
    },
    /// Files in the tree could not be listed.
    #[error("failed to list {} files: {}", reason, output)]
    ListFiles {
        /// What files were requested.
        reason: ListFilesReason,
        /// Output from `git ls-files`.
        output: String,
    },
    /// Deleted files were expected to still be in the work tree.
    #[error("reformatting commit {} {} files: `{}`", commit, reason, paths.iter().format("`, `"))]
    DisallowedFiles {
        /// The commit where the bad behavior occurred.
        commit: CommitId,
        /// The reason for the bad behavior.
        reason: DisallowedFilesReason,
        /// The affected paths.
        paths: Vec<String>,
    },
    /// The source repository is not accessible.
    #[error("the source repository is not accessible")]
    InaccessibleSource {},
    /// Pushing to the remote repository failed.
    #[error("failed to push to {}: {}", url, output)]
    PushFailed {
        /// The remote URL.
        url: String,
        /// Output from `git push`.
        output: String,
    },
    /// Failure to run a formatter.
    #[error("formatter error: {}", source)]
    Formatter {
        /// The source of the error.
        #[from]
        source: FormatterError,
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
    /// Failure to parse a filename from a git tree.
    #[error("git filename extraction error: {}", source)]
    GitFileName {
        /// The source of the error.
        #[from]
        source: git_checks_core::FileNameError,
    },
    /// Failure to extract information from a commit.
    #[error("git commit information error: {}", source)]
    GitCommit {
        /// The source of the error.
        #[from]
        source: git_checks_core::CommitError,
    },
    /// Failure to build result comment content.
    #[error("message building write error: {}", source)]
    BuildComment {
        /// The source of the error.
        source: std::fmt::Error,
    },
}

impl ReformatError {
    fn merge_commit() -> Self {
        ReformatError::MergeCommit {}
    }

    fn list_all_files(output: &[u8]) -> Self {
        ReformatError::ListAllFiles {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn list_all_files_format(line: String) -> Self {
        ReformatError::ListAllFilesFormat {
            line,
        }
    }

    fn list_commits(output: &[u8]) -> Self {
        ReformatError::ListCommits {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn add_files(output: &[u8]) -> Self {
        ReformatError::AddFiles {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn write_tree(output: &[u8]) -> Self {
        ReformatError::WriteTree {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn write_commit_message(source: io::Error) -> Self {
        ReformatError::WriteCommitMessage {
            source,
        }
    }

    fn commit_tree(output: &[u8]) -> Self {
        ReformatError::CommitTree {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn find_commit_tree(commit: CommitId, output: &[u8]) -> Self {
        ReformatError::FindCommitTree {
            commit,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn find_commit_dates(commit: CommitId, output: &[u8]) -> Self {
        ReformatError::FindCommitDates {
            commit,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn reformat_failed(commit: CommitId, paths: Vec<&str>) -> Self {
        ReformatError::ReformatFailed {
            commit,
            paths: paths.into_iter().map(Into::into).collect(),
        }
    }

    fn list_files(reason: ListFilesReason, output: &[u8]) -> Self {
        ReformatError::ListFiles {
            reason,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn disallowed_files(commit: CommitId, reason: DisallowedFilesReason, paths: Vec<&str>) -> Self {
        ReformatError::DisallowedFiles {
            commit,
            reason,
            paths: paths.into_iter().map(Into::into).collect(),
        }
    }

    fn inaccessible_source() -> Self {
        ReformatError::InaccessibleSource {}
    }

    fn push_failed(url: String, output: &[u8]) -> Self {
        ReformatError::PushFailed {
            url,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn build_comment(source: std::fmt::Error) -> Self {
        ReformatError::BuildComment {
            source,
        }
    }
}

type ReformatResult<T> = Result<T, ReformatError>;

/// Information about a commit for formatting purposes.
struct CommitInfo {
    /// Information about the commit for checks.
    commit: Commit,
    /// The authorship date of the commit.
    author_date: DateTime<FixedOffset>,
    /// The commit date of the commit.
    committer_date: DateTime<FixedOffset>,
}

/// Implementation of the `reformat` action.
#[derive(Debug)]
pub struct Reformat {
    /// The context to use for Git actions.
    ctx: GitContext,
    /// The project.
    project: HostedProject,
    /// Formatters to run during the action.
    formatters: Vec<Formatter>,
    /// Whether to push the result or not.
    push_result: bool,
}

impl Reformat {
    /// Create a new reformat action.
    pub fn new(ctx: GitContext, project: HostedProject) -> Self {
        Self {
            ctx,
            project,
            formatters: Vec::new(),
            push_result: true,
        }
    }

    /// Add formatters to the action.
    pub fn add_formatters<I>(&mut self, formatters: I) -> &mut Self
    where
        I: IntoIterator<Item = Formatter>,
    {
        self.formatters.extend(formatters.into_iter());
        self
    }

    /// Whether to push the result back to the source branch or not.
    pub fn push_result(&mut self, push: bool) -> &mut Self {
        self.push_result = push;
        self
    }

    /// Reformat the entire tree through a merge request.
    ///
    /// This method rewrites the entire tree as part of a merge request by rewriting the `HEAD` of
    /// the source branch to have reformatting of the entire repository at once.
    pub fn reformat_repo(&self, mr: &MergeRequest) -> ReformatResult<CommitId> {
        let url = if let Some(ref repo) = mr.source_repo {
            &repo.url
        } else {
            self.send_mr_comment(
                mr,
                "The source repository is not accessible. Has it been deleted or is it a private \
                 repository?",
            );
            return Err(ReformatError::inaccessible_source());
        };

        let rewrite_map = HashMap::new();

        let work_commit = self.construct_work_commit(&mr.commit.id, &rewrite_map)?;

        let commit_info = self.commit_info(&work_commit)?;

        if commit_info.commit.parents.len() > 1 {
            self.send_mr_comment(
                mr,
                "The repository cannot be reformatted using a merge commit. Please use a \
                 non-merge commit.",
            );
            return Err(ReformatError::merge_commit());
        }

        let mut workarea = self.ctx.prepare(&work_commit)?;

        // List all files in the repository.
        let ls_files = workarea
            .git()
            // Force quoting to be on.
            .args(["-c", "core.quotePath=true"])
            .arg("ls-tree")
            .arg("-r")
            .arg(work_commit.as_str())
            .output()
            .map_err(|err| GitError::subcommand("ls-tree", err))?;
        if !ls_files.status.success() {
            return Err(ReformatError::list_all_files(&ls_files.stderr));
        }
        let file_paths = String::from_utf8_lossy(&ls_files.stdout);
        let all_files = file_paths
            .par_lines()
            .filter_map(|line| {
                let mut split = line.splitn(2, ' ');
                if let Some(mode) = split.next() {
                    // Ignore submodules.
                    if mode == "160000" {
                        return None;
                    }

                    // Ignore symlinks.
                    if mode == "120000" {
                        return None;
                    }
                } else {
                    return Some(Err(ReformatError::list_all_files_format(line.into())));
                }

                let mut split = line.splitn(2, '\t');
                Some(if let Some(filename) = split.nth(1) {
                    FileName::new(filename).map_err(Into::into)
                } else {
                    error!(
                        target: "ghostflow/reformat",
                        "git did not give us a proper line from `ls-tree -r`: {}",
                        line,
                    );
                    Err(ReformatError::list_all_files_format(line.into()))
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<ReformatResult<Vec<FileName>>>()?;
        workarea.checkout(&all_files)?;
        let all_files_ref = all_files.iter().collect::<Vec<_>>();

        let new_tree = self.reformat_paths(workarea, all_files_ref, &mr.commit.id, mr)?;
        let new_commit = self.commit_tree(&commit_info, new_tree, &rewrite_map)?;

        if self.push_result {
            self.push_new_head(url, &new_commit, mr, &[])?;
        }
        Ok(new_commit)
    }

    /// Reformat a merge request and push it to its source repository.
    ///
    /// The topology of the topic is kept the same by rewriting commits in order and committing the
    /// reformatted trees by replacing the old parent commit IDs with the newly formed parent IDs.
    pub fn reformat_mr(&self, base: &CommitId, mr: &MergeRequest) -> ReformatResult<CommitId> {
        let url = if let Some(ref repo) = mr.source_repo {
            &repo.url
        } else {
            self.send_mr_comment(
                mr,
                "The source repository is not accessible. Has it been deleted or is it a private \
                 repository?",
            );
            return Err(ReformatError::inaccessible_source());
        };

        let rev_list = self
            .ctx
            .git()
            .arg("rev-list")
            .arg("--reverse")
            .arg("--topo-order")
            .arg(format!("^{}", base))
            .arg(mr.commit.id.as_str())
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !rev_list.status.success() {
            return Err(ReformatError::list_commits(&rev_list.stderr));
        }
        let commits = String::from_utf8_lossy(&rev_list.stdout);

        let mut rewrite_map = HashMap::new();
        let mut empty_commits = Vec::new();

        for commit in commits.lines().map(CommitId::new) {
            let work_commit = self.construct_work_commit(&commit, &rewrite_map)?;

            let commit_info = self.commit_info(&work_commit)?;
            let mut workarea = self.ctx.prepare(&work_commit)?;

            // The `modified_files` call ignores submodules and symlinks.
            let changed_files = commit_info.commit.modified_files();

            // Create the files necessary on the disk.
            let files_to_checkout = changed_files
                .iter()
                .map(|path| path.as_path())
                .chain(
                    self.formatters
                        .iter()
                        .flat_map(|formatter| formatter.config_files.iter().map(AsRef::as_ref)),
                )
                .collect::<Vec<_>>();
            workarea.checkout(&files_to_checkout)?;

            let new_tree = self.reformat_paths(workarea, changed_files, &commit, mr)?;
            let new_commit = self.commit_tree(&commit_info, new_tree, &rewrite_map)?;

            // See if we can remove this commit from the history. We can only do this if the
            // following are true:
            //
            //   - the commit has a single parent (merge commits operate on more than just the code
            //     itself and must be kept for their history editing functionality as well);
            //   - the commit was not empty before the reformatting, but is now empty.
            //
            // A commit which meets both of these is highly likely to have been a "apply formatting
            // fixes" commit and may now be removed from the history. The commit will be saved for
            // later so that it may be mentioned as being dropped later.
            if commit_info.commit.parents.len() == 1
                && !Commit::new(&self.ctx, &commit)?.diffs.is_empty()
                && Commit::new(&self.ctx, &new_commit)?.diffs.is_empty()
            {
                let parent = &commit_info.commit.parents[0];
                let new_parent = rewrite_map.get(parent).unwrap_or(parent).clone();

                // Add it to the set of commits to mention.
                empty_commits.push(commit.clone());
                // Stitch up history to point to the reformatted parent.
                rewrite_map.insert(commit, new_parent);

                continue;
            }

            rewrite_map.insert(commit, new_commit);
        }

        let new_head = rewrite_map.get(&mr.commit.id).unwrap_or(&mr.commit.id);
        if self.push_result {
            self.push_new_head(url, new_head, mr, &empty_commits)?;
        }
        Ok(new_head.clone())
    }

    fn reformat_paths(
        &self,
        workarea: GitWorkArea,
        paths: Vec<&FileName>,
        commit: &CommitId,
        mr: &MergeRequest,
    ) -> ReformatResult<CommitId> {
        // Collect the set of files which fail their formatters.
        let check_ctx = CheckGitContext::new(workarea, mr.author.identity());
        let failed_paths = self
            .formatters
            .iter()
            .map(|formatter| {
                // Run paths handled by the formatters according to their attributes.
                let attr = format!("format.{}", formatter.kind);
                paths
                    .par_iter()
                    .map(|path| {
                        let state = check_ctx.check_attr(&attr, path.as_path())?;
                        match state {
                            AttributeState::Set => {
                                formatter.format_path(check_ctx.workarea(), path, None)
                            },
                            AttributeState::Value(v) => {
                                formatter.format_path(check_ctx.workarea(), path, Some(v))
                            },
                            _ => Ok(None),
                        }
                    })
                    .collect::<Vec<FormatterResult<_>>>()
                    .into_iter()
                    .collect::<FormatterResult<Vec<_>>>()
            })
            .collect::<Vec<FormatterResult<_>>>()
            .into_iter()
            .collect::<FormatterResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .filter_map(|path| path.map(FileName::as_str))
            .unique()
            .collect::<Vec<_>>();

        if !failed_paths.is_empty() {
            self.send_mr_comment(
                mr,
                &format!(
                    "Failed to format the following files in {}:\n\n  - `{}`",
                    commit,
                    failed_paths.iter().join("`\n  - `"),
                ),
            );
            return Err(ReformatError::reformat_failed(commit.clone(), failed_paths));
        }

        self.check_deleted_files(check_ctx.workarea(), &paths, commit, mr)?;
        self.check_untracked_files(check_ctx.workarea(), commit, mr)?;

        let add = check_ctx
            .git()
            .arg("add")
            .args(&paths.iter().map(|path| path.as_path()).collect::<Vec<_>>())
            .output()
            .map_err(|err| GitError::subcommand("add", err))?;
        if !add.status.success() {
            return Err(ReformatError::add_files(&add.stderr));
        }

        let write_tree = check_ctx
            .git()
            .arg("write-tree")
            .output()
            .map_err(|err| GitError::subcommand("write-tree", err))?;
        if !write_tree.status.success() {
            return Err(ReformatError::write_tree(&write_tree.stderr));
        }
        Ok(CommitId::new(
            String::from_utf8_lossy(&write_tree.stdout).trim(),
        ))
    }

    /// Push a new head to the source repository of the merge request.
    ///
    /// This creates a comment about the result of the reformatting.
    fn push_new_head(
        &self,
        url: &str,
        new_head: &CommitId,
        mr: &MergeRequest,
        empty_commits: &[CommitId],
    ) -> ReformatResult<()> {
        if new_head == &mr.commit.id {
            self.send_mr_comment(mr, "This topic is clean and required no reformatting.");
        } else {
            let push = self
                .ctx
                .git()
                .arg("push")
                .arg("--atomic")
                .arg("--porcelain")
                .arg(format!(
                    "--force-with-lease=refs/heads/{}:{}",
                    mr.source_branch, mr.commit.id,
                ))
                .arg(url)
                .arg(format!("{}:refs/heads/{}", new_head, mr.source_branch))
                .output()
                .map_err(|err| GitError::subcommand("push", err))?;
            if !push.status.success() {
                self.send_mr_comment(mr, "Failed to push the reformatted branch.");
                return Err(ReformatError::push_failed(url.into(), &push.stderr));
            }

            let mut msg = "This topic has been reformatted and pushed; please fetch from the \
                           source repository and reset your local branch to continue with further \
                           development on the reformatted commits."
                .to_string();
            if !empty_commits.is_empty() {
                write!(
                    msg,
                    "\n\nThe following commits were empty after reformatting and removed from the \
                     history: {}.",
                    empty_commits.iter().format(", "),
                )
                .map_err(ReformatError::build_comment)?;
            }

            self.send_mr_comment(mr, &msg);
        }

        Ok(())
    }

    /// Commit a tree using the same information as a template commit.
    fn commit_tree(
        &self,
        commit_info: &CommitInfo,
        tree: CommitId,
        rewrite_map: &HashMap<CommitId, CommitId>,
    ) -> ReformatResult<CommitId> {
        let parent_args_paired = iter::repeat("-p")
            .zip(
                commit_info
                    .commit
                    .parents
                    .iter()
                    .map(|parent| rewrite_map.get(parent).unwrap_or(parent).as_str()),
            )
            .map(|(flag, arg)| [flag, arg])
            .collect::<Vec<_>>();
        let parent_args = parent_args_paired.iter().flatten();
        let mut commit_tree = self
            .ctx
            .git()
            .arg("commit-tree")
            .arg(tree.as_str())
            .args(parent_args)
            .env("GIT_AUTHOR_NAME", &commit_info.commit.author.name)
            .env("GIT_AUTHOR_EMAIL", &commit_info.commit.author.email)
            .env("GIT_AUTHOR_DATE", commit_info.author_date.to_rfc2822())
            .env("GIT_COMMITTER_NAME", &commit_info.commit.committer.name)
            .env("GIT_COMMITTER_EMAIL", &commit_info.commit.committer.email)
            .env(
                "GIT_COMMITTER_DATE",
                commit_info.committer_date.to_rfc2822(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| GitError::subcommand("commit-tree (spawn)", err))?;
        let _ = parent_args;

        {
            let commit_tree_stdin = commit_tree
                .stdin
                .as_mut()
                .expect("expected commit-tree to have a stdin");
            commit_tree_stdin
                .write_all(commit_info.commit.message.as_bytes())
                .map_err(ReformatError::write_commit_message)?;
        }

        let commit_tree = commit_tree
            .wait_with_output()
            .map_err(|err| GitError::subcommand("commit-tree (wait)", err))?;
        if !commit_tree.status.success() {
            return Err(ReformatError::commit_tree(&commit_tree.stderr));
        }

        let new_commit = String::from_utf8_lossy(&commit_tree.stdout);
        Ok(CommitId::new(new_commit.trim()))
    }

    /// Construct a commit on which to perform the actual reformatting.
    ///
    /// This is required because we only reformat files which have changed in a particular commit.
    /// Given commit `A` which is an ancestor of commit `B`, if file `X` is reformatted in `A`,
    /// but unchanged in `B` from `A`, the blind reformatting of `B` will inherit the file `X` from
    /// `A`, ignoring the reformatting we just did for `A'`. Instead, create a commit `B''` in the
    /// location of the commit `B'` with the history of the target topic so that when we ask "what
    /// files changed in `B` compared to `A'`?", we see that `X` needs to be reformatted in `B` as
    /// well since it inherited it from `A` (and since it was reformatted in `A'`, there is now a
    /// difference).
    ///
    /// This function takes `B` and creates our working commit `B''` using the new history.
    fn construct_work_commit(
        &self,
        commit: &CommitId,
        rewrite_map: &HashMap<CommitId, CommitId>,
    ) -> ReformatResult<CommitId> {
        let commit_info = self.commit_info(commit)?;
        let rev_parse = self
            .ctx
            .git()
            .arg("rev-parse")
            .arg(format!("{}^{{tree}}", commit))
            .output()
            .map_err(|err| GitError::subcommand("rev-parse", err))?;
        if !rev_parse.status.success() {
            return Err(ReformatError::find_commit_tree(
                commit.clone(),
                &rev_parse.stderr,
            ));
        }
        let tree = CommitId::new(String::from_utf8_lossy(&rev_parse.stdout).trim());
        self.commit_tree(&commit_info, tree, rewrite_map)
    }

    /// Gather the required information about a commit.
    fn commit_info(&self, commit: &CommitId) -> ReformatResult<CommitInfo> {
        let commit_dates = self
            .ctx
            .git()
            .arg("log")
            .arg("--pretty=%aI%n%cI")
            .arg("--max-count=1")
            .arg(commit.as_str())
            .output()
            .map_err(|err| GitError::subcommand("log --pretty=date", err))?;
        if !commit_dates.status.success() {
            return Err(ReformatError::find_commit_dates(
                commit.clone(),
                &commit_dates.stderr,
            ));
        }
        let dates = String::from_utf8_lossy(&commit_dates.stdout);
        let date_lines = dates.lines().collect::<Vec<_>>();

        assert!(
            date_lines.len() == 2,
            "got {} rather than 2 lines when logging a commit: {:?}",
            date_lines.len(),
            date_lines,
        );

        Ok(CommitInfo {
            commit: Commit::new(&self.ctx, commit)?,
            author_date: date_lines[0].parse()?,
            committer_date: date_lines[1].parse()?,
        })
    }

    /// Check the workarea for files that have been deleted.
    fn check_deleted_files(
        &self,
        workarea: &GitWorkArea,
        paths: &[&FileName],
        commit: &CommitId,
        mr: &MergeRequest,
    ) -> ReformatResult<()> {
        // If there were no paths changed, all files are considered deleted because an empty list
        // of files causes `ls-files` to list all files.
        if paths.is_empty() {
            return Ok(());
        }

        let ls_files = workarea
            .git()
            .arg("ls-files")
            .arg("-d")
            .arg("--")
            .args(&paths.iter().map(|path| path.as_path()).collect::<Vec<_>>())
            .output()
            .map_err(|err| GitError::subcommand("ls-files -d", err))?;
        if !ls_files.status.success() {
            return Err(ReformatError::list_files(
                ListFilesReason::Deleted,
                &ls_files.stderr,
            ));
        }
        let ls_files_output = String::from_utf8_lossy(&ls_files.stdout);
        let deleted_paths = ls_files_output.lines().collect::<Vec<_>>();

        if !deleted_paths.is_empty() {
            self.send_mr_comment(
                mr,
                &format!(
                    "The following paths were deleted while formatting {}:\n\n  - `{}`",
                    commit,
                    deleted_paths.iter().join("`\n  - `"),
                ),
            );
            return Err(ReformatError::disallowed_files(
                commit.clone(),
                DisallowedFilesReason::Deleted,
                deleted_paths,
            ));
        }

        Ok(())
    }

    /// Check for untracked files which have been dropped into the workarea.
    fn check_untracked_files(
        &self,
        workarea: &GitWorkArea,
        commit: &CommitId,
        mr: &MergeRequest,
    ) -> ReformatResult<()> {
        let ls_files = workarea
            .git()
            .arg("ls-files")
            .arg("-o")
            .output()
            .map_err(|err| GitError::subcommand("ls-files -o", err))?;
        if !ls_files.status.success() {
            return Err(ReformatError::list_files(
                ListFilesReason::Untracked,
                &ls_files.stderr,
            ));
        }
        let ls_files_output = String::from_utf8_lossy(&ls_files.stdout);
        let untracked_paths = ls_files_output.lines().collect::<Vec<_>>();

        if !untracked_paths.is_empty() {
            self.send_mr_comment(
                mr,
                &format!(
                    "The following untracked paths were created while formatting {}:\n\n  - \
                     `{}`",
                    commit,
                    untracked_paths.iter().join("`\n  - `"),
                ),
            );
            return Err(ReformatError::disallowed_files(
                commit.clone(),
                DisallowedFilesReason::Created,
                untracked_paths,
            ));
        }

        Ok(())
    }

    /// Send a comment to a merge request.
    fn send_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if let Err(err) = self.project.service.post_mr_comment(mr, content) {
            error!(
                target: "ghostflow/reformat",
                "failed to post a comment to merge request: {}, {}: {:?}",
                self.project.name,
                mr.id,
                err,
            );
        }
    }
}
