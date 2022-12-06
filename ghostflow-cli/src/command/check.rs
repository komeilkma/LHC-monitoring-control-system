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

type CheckResult<T> = Result<T, CheckError>;

pub struct Check;

