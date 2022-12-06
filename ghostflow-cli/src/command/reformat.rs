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

impl ReformatError {
    fn unknown_command(subcommand: String) -> Self {
        ReformatError::UnknownCommand {
            subcommand,
        }
    }

    fn invalid_configuration_file(path: PathBuf, source: io::Error) -> Self {
        ReformatError::InvalidConfigurationFile {
            path,
            source,
        }
    }

    fn read_commit_configuration(commit: CommitId, source: GitError) -> Self {
        ReformatError::ReadCommitConfiguration {
            commit,
            source,
        }
    }
}

type ReformatResult<T> = Result<T, ReformatError>;

pub struct Reformat;

impl Reformat {
    pub fn run(service: Arc<dyn LocalService>, matches: &ArgMatches) -> ReformatResult<ExitCode> {
        matches
            .values_of("FORMATTER")
            .map(Formatter::parse_args)
            .transpose()?;

        let ctx = service.git_context().clone();

        let (old_commit, reformatted_commit) = match matches.subcommand() {
            Some(("commits", m)) => Commits::run(service, m)?,
            Some(("repo", m)) => Repo::run(service, m)?,
            Some((subcmd, _)) => return Err(ReformatError::unknown_command(subcmd.into())),
            None => return Err(ReformatError::MissingCommand),
        };
        println!("{}", reformatted_commit);

        let checkout_target = if let Some(refname) = matches.value_of("REF") {
            let full_refname = if refname.starts_with("refs/") {
                refname.into()
            } else {
                format!("{}{}", BRANCH_REF_PREFIX, refname)
            };

            let reason = format!("ghostflow reformat of `{}`", old_commit);

            let update_ref = ctx
                .git()
                .arg("update-ref")
                .args(["-m", &reason])
                .arg(&full_refname)
                .arg(reformatted_commit.as_str())
                .output()
                .map_err(|err| GitError::subcommand("update-ref", err))?;
            if !update_ref.status.success() {
                return Err(ReformatError::GitUpdateRef(
                    String::from_utf8_lossy(&update_ref.stderr).into_owned(),
                ));
            }

            CommitId::new(
                if let Some(branch_name) = full_refname.strip_prefix(BRANCH_REF_PREFIX) {
                    branch_name.into()
                } else {
                    full_refname
                },
            )
        } else {
            reformatted_commit
        };

        if matches.is_present("CHECKOUT") {
            let checkout = ctx
                .git()
                .arg("checkout")
                .arg(checkout_target.as_str())
                .output()
                .map_err(|err| GitError::subcommand("checkout", err))?;
            if !checkout.status.success() {
                return Err(ReformatError::GitCheckout(
                    String::from_utf8_lossy(&checkout.stderr).into_owned(),
                ));
            }
        }

        Ok(ExitCode::Success)
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("reformat")
            .about("reformat code in a repository")
            .arg(
                Arg::new("FORMATTER")
                    .short('F')
                    .long("formatter")
                    .help("specify the path to a formatter `KIND=PATH`")
                    .takes_value(true)
                    .number_of_values(1)
                    .multiple_occurrences(true),
            )
            .arg(
                Arg::new("REF")
                    .short('r')
                    .long("ref")
                    .help("Update a ref with the new commit")
                    .takes_value(true),
            )
            .arg(
                Arg::new("CHECKOUT")
                    .short('C')
                    .long("checkout")
                    .help("`git checkout` the new commit (or ref)")
                    .takes_value(false),
            )
            .subcommand(Commits::subcommand())
            .subcommand(Repo::subcommand())
    }
}

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
