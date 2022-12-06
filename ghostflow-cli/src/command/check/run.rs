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
type RunResult<T> = Result<T, RunError>;

