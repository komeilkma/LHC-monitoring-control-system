//! The `test` action using ref-based testing.
//!
//! This action pushes refs into a ref namespace for use by testing machines.

use git_workarea::{GitContext, GitError};
use log::{error, info, warn};
use thiserror::Error;

use crate::host::{CommitStatusState, HostedProject, HostingServiceError, MergeRequest};

/// Operations on a test ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRefOp {
    /// Update the test ref.
    Update,
    /// Push the test ref.
    Push,
    /// Delete a local test ref.
    DeleteLocal,
    /// Delete a remote test ref.
    DeleteRemote,
}

impl TestRefOp {
    fn desc(self) -> &'static str {
        match self {
            TestRefOp::Update => "update test ref",
            TestRefOp::Push => "push test ref",
            TestRefOp::DeleteLocal => "delete local test ref",
            TestRefOp::DeleteRemote => "delete remote test ref",
        }
    }
}

/// Errors which may occur when handling test refs for a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TestRefsError {
    /// Failure to operate on a test ref.
    #[error("failed to {} `{}`: {}", op.desc(), refname, output)]
    UpdateRef {
        /// The test refname.
        refname: String,
        /// The operation.
        op: TestRefOp,
        /// The output from the `git` command.
        output: String,
    },
    /// Failure to list test refs.
    #[error("failed to list test refs under {}: {}", refnamespace, output)]
    ListRefs {
        /// The namespace of test refs.
        refnamespace: String,
        /// The output of `git for-each-ref`.
        output: String,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
}

impl TestRefsError {
    fn update_ref(refname: String, op: TestRefOp, output: &[u8]) -> Self {
        TestRefsError::UpdateRef {
            refname,
            op,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn list_refs(refnamespace: String, output: &[u8]) -> Self {
        TestRefsError::ListRefs {
            refnamespace,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type TestRefsResult<T> = Result<T, TestRefsError>;

/// Implementation of the `test` action.
#[derive(Debug)]
pub struct TestRefs {
    /// The context to use for Git actions.
    ctx: GitContext,
    /// The project to be tested.
    project: HostedProject,
    /// The namespace to use for test refs.
    namespace: String,
    /// Whether the action should create informational comments or not.
    ///
    /// Errors always create comments.
    quiet: bool,
}

impl TestRefs {
    /// Create a new test action.
    pub fn new(ctx: GitContext, project: HostedProject) -> Self {
        Self {
            ctx,
            project,
            namespace: "test-topics".into(),
            quiet: false,
        }
    }

    /// Reduce the number of comments made by the test action.
    pub fn quiet(&mut self) -> &mut Self {
        self.quiet = true;
        self
    }

    /// The ref namespace to use for test topics.
    pub fn ref_namespace<N>(&mut self, namespace: N) -> &mut Self
    where
        N: Into<String>,
    {
        self.namespace = namespace.into();
        self
    }

    /// Push a merge request for testing.
    pub fn test_mr(&self, mr: &MergeRequest) -> TestRefsResult<()> {
        info!(
            target: "ghostflow/test/refs",
            "pushing a test ref for {}",
            mr.url,
        );

        // Fetch the merge request into the stager's git context.
        self.project.service.fetch_mr(&self.ctx, mr)?;

        let refname = self.refname(mr);

        let update_ref = self
            .ctx
            .git()
            .arg("update-ref")
            .arg(&refname)
            .arg(mr.commit.id.as_str())
            .output()
            .map_err(|err| GitError::subcommand("update-ref", err))?;
        if !update_ref.status.success() {
            return Err(TestRefsError::update_ref(
                refname,
                TestRefOp::Update,
                &update_ref.stderr,
            ));
        }

        let push = self
            .ctx
            .git()
            .arg("push")
            .arg("origin")
            .arg("--atomic")
            .arg("--porcelain")
            .arg(format!("{}:{}", refname, refname))
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !push.status.success() {
            return Err(TestRefsError::update_ref(
                refname,
                TestRefOp::Push,
                &update_ref.stderr,
            ));
        }

        self.send_info_mr_comment(mr, "This topic has been pushed for testing.");

        self.send_mr_commit_status(mr, CommitStatusState::Success, "pushed for testing");

        Ok(())
    }

    /// Remove a merge request from the testing set.
    pub fn untest_mr(&self, mr: &MergeRequest) -> TestRefsResult<()> {
        info!(
            target: "ghostflow/test/refs",
            "deleting the test ref for {}",
            mr.url,
        );

        let refname = self.refname(mr);

        let show_ref = self
            .ctx
            .git()
            .arg("show-ref")
            .arg("--quiet")
            .arg("--verify")
            .arg(&refname)
            .status()
            .map_err(|err| GitError::subcommand("show-ref", err))?;
        if !show_ref.success() {
            // There is no such ref; skip.
            return Ok(());
        }

        self.delete_ref(&refname)?;

        let push = self
            .ctx
            .git()
            .arg("push")
            .arg("origin")
            .arg("--atomic")
            .arg("--porcelain")
            .arg(format!(":{}", refname))
            .output()
            .map_err(|err| GitError::subcommand("push :refname", err))?;
        if !push.status.success() {
            return Err(TestRefsError::update_ref(
                refname,
                TestRefOp::DeleteRemote,
                &push.stderr,
            ));
        }

        self.send_mr_commit_status(mr, CommitStatusState::Success, "removed from testing");

        Ok(())
    }

    /// Clear the set of merge requests for testing.
    pub fn clear_all_mrs(&self) -> TestRefsResult<()> {
        info!(
            target: "ghostflow/test/refs",
            "clearing all test refs for {}",
            self.project.name,
        );

        let test_refs = self
            .ctx
            .git()
            .arg("for-each-ref")
            .arg("--format=%(refname:strip=2)")
            .arg(format!("refs/{}/", self.namespace))
            .output()
            .map_err(|err| GitError::subcommand("for-each-ref", err))?;
        if !test_refs.status.success() {
            return Err(TestRefsError::list_refs(
                self.namespace.clone(),
                &test_refs.stderr,
            ));
        }
        let topic_ids = String::from_utf8_lossy(&test_refs.stdout);
        topic_ids
            .lines()
            .filter_map(|topic_id| {
                match topic_id.parse() {
                    Ok(id) => Some(id),
                    Err(err) => {
                        error!(
                            target: "ghostflow/test/refs",
                            "failed to parse {} as a topic id; deleting the ref: {:?}",
                            topic_id,
                            err,
                        );

                        let refname = format!("refs/{}/{}", self.namespace, topic_id);
                        self.lenient_delete_ref(refname);

                        None
                    },
                }
            })
            .filter_map(|topic_id| {
                match self.project.merge_request(topic_id) {
                    Ok(mr) => Some(self.untest_mr(&mr)),
                    Err(err) => {
                        error!(
                            target: "ghostflow/test/refs",
                            "ref {} is not a valid merge request; deleting the ref: {:?}",
                            topic_id,
                            err,
                        );

                        let refname = format!("refs/{}/{}", self.namespace, topic_id);
                        self.lenient_delete_ref(refname);

                        None
                    },
                }
            })
            .collect::<TestRefsResult<Vec<_>>>()?;

        Ok(())
    }

    /// The refname for a merge request.
    fn refname(&self, mr: &MergeRequest) -> String {
        format!("refs/{}/{}", self.namespace, mr.id)
    }

    /// Delete a test ref from the local repository.
    fn delete_ref(&self, refname: &str) -> TestRefsResult<()> {
        info!(
            target: "ghostflow/test/refs",
            "deleting test ref {}",
            refname,
        );

        let delete_ref = self
            .ctx
            .git()
            .arg("update-ref")
            .arg("-d")
            .arg(refname)
            .output()
            .map_err(|err| GitError::subcommand("update-ref -d", err))?;
        if !delete_ref.status.success() {
            return Err(TestRefsError::update_ref(
                refname.into(),
                TestRefOp::DeleteLocal,
                &delete_ref.stderr,
            ));
        }

        Ok(())
    }

    /// Delete a test ref, ignoring errors.
    fn lenient_delete_ref(&self, refname: String) {
        let _ = self.delete_ref(&refname).map_err(|err| {
            error!(
                target: "ghostflow/test/refs",
                "failed to delete the {} ref from {}: {:?}",
                refname,
                self.project.name,
                err,
            );
        });
    }

    /// Set the commit status to a merge request.
    fn send_mr_commit_status(&self, mr: &MergeRequest, status: CommitStatusState, desc: &str) {
        let status = mr.create_commit_status(status, "ghostflow-test", desc);
        if let Err(err) = self.project.service.post_commit_status(status) {
            warn!(
                target: "ghostflow/test/refs",
                "failed to post a commit status for mr {} on {} for '{}': {:?}",
                mr.id,
                mr.commit.id,
                desc,
                err,
            );
        }
    }

    /// Send a comment to a merge request.
    fn send_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if let Err(err) = self.project.service.post_mr_comment(mr, content) {
            error!(
                target: "ghostflow/test/refs",
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
