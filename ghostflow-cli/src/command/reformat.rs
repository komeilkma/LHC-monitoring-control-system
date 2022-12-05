use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};
use ghostflow::actions::reformat;
use git_workarea::{CommitId, GitError};
use thiserror::Error;

use crate::checks::formatter::{Formatter, FormatterError};
use crate::config::ConfigError;
use crate::exit_code::ExitCode;
use crate::host::LocalService;

mod config;
pub use self::config::{Config, Read};

mod commits;
use self::commits::{Commits, CommitsError};

mod repo;
use self::repo::{Repo, RepoError};

const BRANCH_REF_PREFIX: &str = "refs/heads/";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReformatError {
    #[error("the `reformat` subcommand requires a subcommand")]
    MissingCommand,
    #[error("unknown `reformat` subcommand `{}`", subcommand)]
    UnknownCommand { subcommand: String },
    #[error("failed to read configuration file at `{}`: {}", path.display(), source)]
    InvalidConfigurationFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read configuration from commit {}: {}", commit, source)]
    ReadCommitConfiguration {
        commit: CommitId,
        #[source]
        source: GitError,
    },
    #[error("git update-ref failed: {}", _0)]
    GitUpdateRef(String),
    #[error("git checkout failed: {}", _0)]
    GitCheckout(String),
    #[error("formatter error: {}", source)]
    Formatter {
        #[from]
        source: FormatterError,
    },
    #[error("reformat commits error: {}", source)]
    Commits {
        #[from]
        source: CommitsError,
    },
    #[error("reformat repo error: {}", source)]
    Repo {
        #[from]
        source: RepoError,
    },
    #[error("git error: {}", source)]
    Git {
        #[from]
        source: GitError,
    },
    #[error("reformatter action error: {}", source)]
    ReformatterAction {
        #[from]
        source: reformat::ReformatError,
    },
    #[error("config error: {}", source)]
    Config {
        #[from]
        source: ConfigError,
    },
}


type ReformatResult<T> = Result<T, ReformatError>;

pub struct Reformat;

fn config_from_args(
    local_service: &dyn LocalService,
    commit: &CommitId,
    matches: &ArgMatches,
) -> ReformatResult<Option<Config>> {
    let conf_data = if let Some(config) = matches.value_of("CONFIG") {
        // Read the configuration from the file.
        Some(
            fs::read(config)
                .map_err(|err| ReformatError::invalid_configuration_file(config.into(), err))?,
        )
    } else if let Some(config_commit) = matches.value_of("CONFIG_COMMIT") {
        let config_commit = match config_commit {
            "HEAD" => commit.clone(),
            rev => CommitId::new(rev),
        };

        // Read the configuration from the target branch.
        local_service
            .config(&config_commit)
            .map_err(|err| ReformatError::read_commit_configuration(config_commit, err))?
    } else {
        None
    };

    conf_data
        .map(|data| {
            let conf = crate::config::Config::from_bytes(data)?;
            Ok(conf.reformat)
        })
        .transpose()
}
