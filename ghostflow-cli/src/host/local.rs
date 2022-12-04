use std::env;
use std::ffi::OsString;
use std::io;
use std::path::Path;

use ghostflow::host::{Repo, User};
use git_workarea::GitContext;
use termcolor::ColorChoice;
use thiserror::Error;

mod ghostflow_impl;
mod git_color;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LocalError {
    #[error("ghostflow does not support git repository paths that are not valid UTF-8")]
    NonUnicodePath,
    #[error("failed to execute `git config`: {}", source)]
    GitConfigExec {
        #[source]
        source: io::Error,
    },
    #[error("`git config` failed with exit code {}", _0)]
    GitConfigError(i32),
    #[error("`git config` exited via a signal")]
    GitConfigSignal,
    #[error("`{}` environment variable was not valid unicode (`{}`)", var, value)]
    NotUnicodeVariable { var: String, value: String },
    #[error("`user.name` is not set")]
    MissingName,
    #[error("`user.email` is not set")]
    MissingEmail,
}

impl LocalError {
    fn git_config_exec(source: io::Error) -> Self {
        LocalError::GitConfigExec {
            source,
        }
    }

    fn invalid_variable(var: String, value: OsString) -> Self {
        LocalError::NotUnicodeVariable {
            var,
            value: value.to_string_lossy().into(),
        }
    }
}

type LocalResult<T> = Result<T, LocalError>;

pub struct Local {
    ctx: GitContext,
    repo: Repo,
    user: User,

    color: ColorChoice,
}

const PROJECT_NAME: &str = "ghostflow.local";

impl Local {
    pub fn new<P>(repo: P, color: ColorChoice) -> LocalResult<Self>
    where
        P: AsRef<Path>,
    {
        let url = repo
            .as_ref()
            .to_str()
            .ok_or(LocalError::NonUnicodePath)?
            .into();
        let ctx = GitContext::new(repo.as_ref());

        let repo = Repo {
            name: PROJECT_NAME.into(),
            url,
            forked_from: None,
        };

        let handle =
            Self::_git_config(&ctx, "ghostflow.handle")?.unwrap_or_else(|| "ghostflow".into());
        let name = match Self::_env_or_config(&ctx, "GIT_AUTHOR_NAME", "user.name")? {
            Some(name) => name,
            None => return Err(LocalError::MissingName),
        };
        let email = match Self::_env_or_config(&ctx, "GIT_AUTHOR_EMAIL", "user.email")? {
            Some(email) => email,
            None => return Err(LocalError::MissingEmail),
        };
        let user = User {
            handle,
            name,
            email,
        };

        Ok(Self {
            ctx,
            repo,
            user,
            color,
        })
    }

    fn git_config(&self, config: &str) -> LocalResult<Option<String>> {
        Self::_git_config(&self.ctx, config)
    }

    fn _env_or_config(ctx: &GitContext, envvar: &str, config: &str) -> LocalResult<Option<String>> {
        match env::var(envvar) {
            Ok(name) => Ok(Some(name)),
            Err(env::VarError::NotUnicode(name)) => {
                Err(LocalError::invalid_variable(envvar.into(), name))
            },
            Err(env::VarError::NotPresent) => Self::_git_config(ctx, config),
        }
    }

    fn _git_config(ctx: &GitContext, config: &str) -> LocalResult<Option<String>> {
        let config = ctx
            .git()
            .arg("config")
            .arg("--get")
            .arg(config)
            .output()
            .map_err(LocalError::git_config_exec)?;

        Ok(match config.status.code() {
            Some(0) => {
                let mut value = String::from_utf8_lossy(&config.stdout).into_owned();

                // Remove trailing whitespace from the value.
                let non_ws_len = value.trim_end().len();
                value.truncate(non_ws_len);

                Some(value)
            },
            Some(1) => None,
            Some(x) => return Err(LocalError::GitConfigError(x)),
            None => return Err(LocalError::GitConfigSignal),
        })
    }
}
