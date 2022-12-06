use std::sync::Arc;

use clap::{ArgMatches, Command};
use thiserror::Error;

use crate::exit_code::ExitCode;
use crate::host::LocalService;

mod config;
pub use self::config::{Config, ConfigError, Read};

mod list;
use self::list::{List, ListError};

mod run;
use self::run::{Run, RunError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CheckError {
    #[error("the `check` subcommand requires a subcommand")]
    MissingCommand,
    #[error("unknown `check` subcommand `{}`", subcommand)]
    UnknownCommand { subcommand: String },
    #[error("{}", source)]
    List {
        #[from]
        source: ListError,
    },
    #[error("{}", source)]
    Run {
        #[from]
        source: RunError,
    },
}

impl CheckError {
    fn unknown_command(subcommand: String) -> Self {
        CheckError::UnknownCommand {
            subcommand,
        }
    }
}

type CheckResult<T> = Result<T, CheckError>;

pub struct Check;

impl Check {
    pub fn run(service: Arc<dyn LocalService>, matches: &ArgMatches) -> CheckResult<ExitCode> {
        match matches.subcommand() {
            Some(("list", m)) => Ok(List::run(m)?),
            Some(("run", m)) => Ok(Run::run(service, m)?),
            Some((subcmd, _)) => Err(CheckError::unknown_command(subcmd.into())),
            None => Err(CheckError::MissingCommand),
        }
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("check")
            .about("manage checks on a repository")
            .subcommand(List::subcommand())
            .subcommand(Run::subcommand())
    }
}
