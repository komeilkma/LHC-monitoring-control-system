//! ghostflow-cli
//!
//! This is a command line tool which may be used to perform workflow actions on a repository.

#![cfg_attr(feature = "doc", feature(external_doc))]

#![cfg_attr(feature = "doc", feature(external_doc))]

use std::num::ParseIntError;
use std::process::Command;
use std::sync::Arc;

use clap::Arg;
use git_workarea::GitError;
use log::LevelFilter;
use rayon::ThreadPoolBuilder;
use termcolor::ColorChoice;
use thiserror::Error;

mod exit_code;
use exit_code::ExitCode;

#[cfg_attr(feature = "doc", doc(include = "../doc/checks.md"))]
pub mod checks;

#[cfg_attr(feature = "doc", doc(include = "../doc/formatters.md"))]
pub mod formatters {}

mod config;

mod host;
use host::{Ci, CiError, Local, LocalError, LocalService};

mod command;
use command::check::{Check, CheckError};
use command::reformat::{Reformat, ReformatError};

#[derive(Debug, Error)]
#[non_exhaustive]
enum SetupError {
    #[error("unrecognized logger: `{}`", logger)]
    UnrecognizedLogger { logger: String },
    #[error("non-integer thread count {}: {}", count, source)]
    NonIntegerThreadCount {
        count: String,
        #[source]
        source: ParseIntError,
    },
    #[error("unknown --color value '{}'", color)]
    UnknownColor { color: String },
    #[error("failed to find `.git` directory: {}", output)]
    FindGitDir { output: String },
    #[error("unknown command '{}'", command)]
    UnknownCommand { command: String },
    #[error("failed to initialize the global rayon thread pool: {}", source)]
    RayonThreadPoolInit {
        #[from]
        source: rayon::ThreadPoolBuildError,
    },
    #[error("failed to initialize the local service: {}", source)]
    ServiceInit {
        #[from]
        source: LocalError,
    },
    #[error("failed to initialize CI service: {}", source)]
    Ci {
        #[from]
        source: CiError,
    },
    #[error("git error: {}", source)]
    Git {
        #[from]
        source: GitError,
    },
    #[error("`check` error: {}", source)]
    Check {
        #[from]
        source: CheckError,
    },
    #[error("`reformat` error: {}", source)]
    Reformat {
        #[from]
        source: ReformatError,
    },
}

impl SetupError {
    fn unrecognized_logger(logger: String) -> Self {
        SetupError::UnrecognizedLogger {
            logger,
        }
    }

    fn non_integer_thread_count(count: String, source: ParseIntError) -> Self {
        SetupError::NonIntegerThreadCount {
            count,
            source,
        }
    }

    fn unknown_color(color: String) -> Self {
        SetupError::UnknownColor {
            color,
        }
    }

    fn find_git_dir(output: &[u8]) -> Self {
        SetupError::FindGitDir {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn unknown_command(command: String) -> Self {
        SetupError::UnknownCommand {
            command,
        }
    }
}

enum Logger {
    Env,
}

fn try_main() -> Result<ExitCode, SetupError> {
    let matches = clap::Command::new("ghostflow-cli")
        .version(clap::crate_version!())
        .author("Komeil Majidi <komeilkma@gmail.com>")
        .about("Perform ghostflow actions")
        .arg(
            Arg::new("DEBUG")
                .short('d')
                .long("debug")
                .help("Increase verbosity")
                .multiple_occurrences(true),
        )
        .arg(
            Arg::new("LOGGER")
                .short('l')
                .long("logger")
                .default_value("env")
                .possible_values(["env"])
                .help("Logging backend")
                .value_name("LOGGER")
                .takes_value(true),
        )
        .arg(
            Arg::new("THREAD_COUNT")
                .short('j')
                .long("threads")
                .help("Number of threads to use in the Rayon thread pool")
                .takes_value(true),
        )
        .arg(
            Arg::new("CI")
                .short('c')
                .long("ci")
                .help("Continuous integration environment")
                .takes_value(true)
                // CI environments tell us where the repository is.
                .conflicts_with("REPOSITORY")
                // Color only really matters with a TTY.
                .conflicts_with("COLOR"),
        )
        .arg(
            Arg::new("REPOSITORY")
                .short('r')
                .long("repo")
                .help("Path to the repository")
                .default_value(".")
                .takes_value(true),
        )
        .arg(
            Arg::new("COLOR")
                .long("colors")
                .help("Use colors when writing output")
                .default_value("auto")
                .possible_values(["always", "always-ansi", "false", "never", "auto", "true"])
                .takes_value(true),
        )
        .subcommand(Check::subcommand())
        .subcommand(Reformat::subcommand())
        .get_matches();

    let log_level = match matches.occurrences_of("DEBUG") {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let _logger = match matches
        .value_of("LOGGER")
        .expect("logger should have a value")
    {
        "env" => {
            env_logger::Builder::new().filter(None, log_level).init();
            Logger::Env
        },

        logger => {
            return Err(SetupError::unrecognized_logger(logger.into()));
        },
    };

    log::set_max_level(log_level);

    if let Some(count) = matches.value_of("THREAD_COUNT") {
        let count = count
            .parse::<usize>()
            .map_err(|err| SetupError::non_integer_thread_count(count.into(), err))?;

        ThreadPoolBuilder::new().num_threads(count).build_global()?;
    }

    let service: Arc<dyn LocalService> = if let Some(service) = matches.value_of("CI") {
        Ci::create(service)?.into()
    } else {
        let repo = matches
            .value_of("REPOSITORY")
            .expect("--repo has a default");
        let gitdir = {
            let rev_parse = Command::new("git")
                .arg("rev-parse")
                .arg("--absolute-git-dir")
                .current_dir(repo)
                .output()
                .map_err(|err| GitError::subcommand("rev-parse", err))?;
            if !rev_parse.status.success() {
                return Err(SetupError::find_git_dir(&rev_parse.stderr));
            }

            String::from_utf8_lossy(&rev_parse.stdout)
                .trim_end()
                .to_string()
        };
        let color_choice = matches.value_of("COLOR").expect("--color has a default");
        let color_choice = match color_choice {
            "always" => ColorChoice::Always,
            "always-ansi" => ColorChoice::AlwaysAnsi,
            "false" | "never" => ColorChoice::Never,
            "true" | "auto" => ColorChoice::Auto,
            color => return Err(SetupError::unknown_color(color.into())),
        };

        Arc::new(Local::new(gitdir, color_choice)?)
    };

    let status = match matches.subcommand() {
        Some(("check", m)) => Check::run(service, m)?,
        Some(("reformat", m)) => Reformat::run(service, m)?,
        Some((subcmd, _)) => return Err(SetupError::unknown_command(subcmd.into())),
        None => ExitCode::Success,
    };

    Ok(status)
}

fn main() {
    match try_main() {
        Ok(code) => {
            code.exit();
            unreachable!()
        },
        Err(err) => panic!("{:?}", err),
    }
}
