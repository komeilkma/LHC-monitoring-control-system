use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};
use thiserror::Error;

use crate::checks::formatter::{Formatter, FormatterError};
use crate::exit_code::ExitCode;
use crate::host::LocalService;

mod commits;
use self::commits::{Commits, CommitsError};

mod topic;
use self::topic::{Topic, TopicError};

pub struct Run;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RunError {
    #[error("the `check run` subcommand requires a subcommand")]
    NoSubcommand,
    #[error("unknown `check run` subcommand `{}`", subcommand)]
    UnknownCommand { subcommand: String },
    #[error("formatter error: {}", source)]
    Formatter {
        #[from]
        source: FormatterError,
    },
    #[error("{}", source)]
    Commits {
        #[from]
        source: CommitsError,
    },
    #[error("{}", source)]
    Topic {
        #[from]
        source: TopicError,
    },
}

impl RunError {
    fn unknown_command(subcommand: String) -> Self {
        RunError::UnknownCommand {
            subcommand,
        }
    }
}

type RunResult<T> = Result<T, RunError>;

impl Run {
    pub fn run(service: Arc<dyn LocalService>, matches: &ArgMatches) -> RunResult<ExitCode> {
        matches
            .values_of("FORMATTER")
            .map(Formatter::parse_args)
            .transpose()?;

        let config = matches.value_of("CONFIG");

        match matches.subcommand() {
            Some(("commits", m)) => Ok(Commits::run(service, m, config)?),
            Some(("topic", m)) => Ok(Topic::run(service, m, config)?),
            Some((subcmd, _)) => Err(RunError::unknown_command(subcmd.into())),
            None => Err(RunError::NoSubcommand),
        }
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("run")
            .about("run checks")
            .arg(
                Arg::new("CONFIG")
                    .short('c')
                    .long("config")
                    .help("Configuration for checks")
                    .takes_value(true),
            )
            .arg(
                Arg::new("FORMATTER")
                    .short('F')
                    .long("formatter")
                    .help("specify the path to a formatter `KIND=PATH`")
                    .takes_value(true)
                    .number_of_values(1)
                    .multiple_occurrences(true),
            )
            .subcommand(Commits::subcommand())
            .subcommand(Topic::subcommand())
    }
}
