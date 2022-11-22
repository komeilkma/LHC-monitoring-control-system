
//! The `follow` action.
//!
//! This action pushes into a ref namespace on the remote that tracks a branch at a coarser
//! interval than every commit. Its intended use case is to keep a stable reference across a longer
//! timespan so that asynchronous external tools all use the same commit.

use git_workarea::{GitContext, GitError};
use log::info;
use thiserror::Error;

/// Errors which may occur when updating a follow ref.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FollowError {
    /// Failure to push a follow ref.
    #[error("failed to push {} into {}: {}", branch, refname, output)]
    Push {
        /// The branch to follow.
        branch: String,
        /// The refname to push it to.
        refname: String,
        /// Output from `git push`.
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

impl FollowError {
    fn push(branch: String, refname: String, output: &[u8]) -> Self {
        FollowError::Push {
            branch,
            refname,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type FollowResult<T> = Result<T, FollowError>;

/// Implementation of the `follow` action.
#[derive(Debug)]
pub struct Follow {
    /// The context to use for fetching data refs.
    ctx: GitContext,
    /// The branch to follow.
    branch: String,
    /// The reference namespace to use for data.
    ref_namespace: String,
}

impl Follow {
    /// Create a new follow action.
    pub fn new<B>(ctx: GitContext, branch: B) -> Self
    where
        B: Into<String>,
    {
        Self {
            ctx,
            branch: branch.into(),
            ref_namespace: "follow".into(),
        }
    }

    /// Use the given ref namespace for follow refs.
    ///
    /// By default, `follow` is used to push under `refs/follow/`.
    pub fn ref_namespace<R>(&mut self, ref_namespace: R) -> &mut Self
    where
        R: Into<String>,
    {
        self.ref_namespace = ref_namespace.into();
        self
    }

    /// Update the remote ref using the given name.
    pub fn update<N>(&self, name: N) -> FollowResult<()>
    where
        N: AsRef<str>,
    {
        self.update_impl(name.as_ref())
    }

    /// Non-generic version of `update`.
    fn update_impl(&self, name: &str) -> FollowResult<()> {
        info!(
            target: "ghostflow/follow",
            "following {} into {}",
            self.branch,
            name,
        );

        let refname = format!("refs/{}/{}/{}", self.ref_namespace, self.branch, name);

        let push = self
            .ctx
            .git()
            .arg("push")
            .arg("--atomic")
            .arg("--porcelain")
            .arg("origin")
            .arg(format!("+refs/heads/{}:{}", self.branch, refname))
            .output()
            .map_err(|err| GitError::subcommand("push", err))?;
        if !push.status.success() {
            return Err(FollowError::push(
                self.branch.clone(),
                refname,
                &push.stderr,
            ));
        }

        Ok(())
    }
}
