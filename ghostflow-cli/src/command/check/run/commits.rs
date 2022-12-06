use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};
use ghostflow::host::{CommitStatusState, HostingServiceError};
use git_checks_core::{GitCheckConfiguration, RunError};
use git_workarea::{CommitId, GitError};
use itertools::Itertools;
use rayon::prelude::*;
use thiserror::Error;

use crate::config::{Config, ConfigError};
use crate::exit_code::ExitCode;
use crate::host::LocalService;

pub struct Commits;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CommitsError {
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
    #[error("failed to run rev-list: {}", output)]
    RevList { output: String },
    #[error("failed to run the checks on {}: {}", commit, source)]
    Check {
        commit: CommitId,
        #[source]
        source: RunError,
    },
    #[error("failed to run git: {}", source)]
    Git {
        #[from]
        source: GitError,
    },
    #[error("failed to parse configuration: {}", source)]
    Config {
        #[from]
        source: ConfigError,
    },
    #[error("hosting service error: {}", source)]
    Service {
        #[from]
        source: HostingServiceError,
    },
    #[error("message building write error: {}", source)]
    BuildComment { source: std::fmt::Error },
}

impl CommitsError {
    fn invalid_configuration_file(path: PathBuf, source: io::Error) -> Self {
        CommitsError::InvalidConfigurationFile {
            path,
            source,
        }
    }

    fn read_commit_configuration(commit: CommitId, source: GitError) -> Self {
        CommitsError::ReadCommitConfiguration {
            commit,
            source,
        }
    }

    fn rev_list(output: &[u8]) -> Self {
        CommitsError::RevList {
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn check(commit: CommitId, source: RunError) -> Self {
        CommitsError::Check {
            commit,
            source,
        }
    }

    fn build_comment(source: std::fmt::Error) -> Self {
        Self::BuildComment {
            source,
        }
    }
}

type CommitsResult<T> = Result<T, CommitsError>;

impl Commits {
    pub fn run(
        service: Arc<dyn LocalService>,
        matches: &ArgMatches,
        config: Option<&str>,
    ) -> CommitsResult<ExitCode> {
        let mut refspecs: Vec<_> = matches
            .values_of("COMMIT")
            .unwrap_or_default()
            .map(Cow::Borrowed)
            .collect();

        if refspecs.is_empty() {
            refspecs.push(Cow::Borrowed("HEAD"));
        }

        if let Some(target) = matches.value_of("TARGET") {
            refspecs.push(Cow::Owned(format!("^{}", target)));
        }

        let conf_data = if let Some(config) = config {
            // Read the configuration from the file.
            Some(
                fs::read(config)
                    .map_err(|err| CommitsError::invalid_configuration_file(config.into(), err))?,
            )
        } else {
            None
        };
        let conf;
        let check_conf = if let Some(data) = conf_data {
            conf = Config::from_bytes(data)?;
            Some(conf.checks.commit_check_configuration())
        } else {
            None
        };

        let ctx = service.git_context();
        let rev_list = ctx
            .git()
            .arg("rev-list")
            .arg("--reverse")
            .arg("--topo-order")
            .args(refspecs.iter().map(Cow::as_ref))
            .output()
            .map_err(|err| GitError::subcommand("rev-list", err))?;
        if !rev_list.status.success() {
            return Err(CommitsError::rev_list(&rev_list.stderr));
        }
        let revs = String::from_utf8_lossy(&rev_list.stdout);
        let identity = service.service_user().identity();
        let quiet = matches.is_present("QUIET");

        let checks = revs
            .par_lines()
            .map(|rev| {
                let rev = CommitId::new(rev);
                let commit_conf;
                let commit_checks = if let Some(ref conf) = check_conf {
                    conf.clone()
                } else {
                    let conf_data = service
                        .config(&rev)
                        .map_err(|err| CommitsError::read_commit_configuration(rev.clone(), err))?;
                    if let Some(data) = conf_data {
                        commit_conf = Config::from_bytes(data)?;
                        commit_conf.checks.commit_check_configuration()
                    } else {
                        GitCheckConfiguration::new()
                    }
                };

                let res = commit_checks
                    .run_commit(ctx, &rev, &identity)
                    .map_err(|err| CommitsError::check(rev.clone(), err))?;

                Ok((rev, res))
            })
            // ::<Fallible<Vec<_>>> would be possible, but makes errors non-deterministic.
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<CommitsResult<Vec<_>>>()?;

        let mut code = ExitCode::Success;
        for (rev, res) in checks {
            let commit = service.synth_commit(&rev)?;
            let state = if res.allowed() || res.pass() {
                if quiet {
                    continue;
                }
                CommitStatusState::Success
            } else {
                code = ExitCode::Failure;
                CommitStatusState::Failed
            };

            let mut desc = String::new();

            // This scope is necessary so that the borrow in `push_results` ends before we use
            // `comment` again at the end of the function.
            {
                let mut push_results = |label, items: &Vec<String>| -> CommitsResult<()> {
                    if !items.is_empty() {
                        write!(desc, "{}:\n  - {}\n", label, items.iter().join("\n  - "))
                            .map_err(CommitsError::build_comment)?;
                    }

                    Ok(())
                };

                push_results("Errors", res.errors())?;
                push_results("Warnings", res.warnings())?;
                push_results("Alerts", res.alerts())?;
            }

            let status = commit.create_commit_status(state, "gf", &desc);
            service.post_commit_status(status)?;
        }

        Ok(code)
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("commits")
            .about("check commits")
            .arg(
                Arg::new("TARGET")
                    .short('t')
                    .long("target")
                    .help("Target branch for integration")
                    .takes_value(true),
            )
            .arg(
                Arg::new("QUIET")
                    .short('q')
                    .long("quiet")
                    .help("Only show errors"),
            )
            .arg(
                Arg::new("COMMIT")
                    .help("Commits to check")
                    .takes_value(true)
                    .multiple_occurrences(true),
            )
    }
}
