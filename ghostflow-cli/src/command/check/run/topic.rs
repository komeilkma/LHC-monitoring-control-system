use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};
use ghostflow::actions::check;
use ghostflow::host::HostingServiceError;
use git_checks_core::GitCheckConfiguration;
use git_workarea::{CommitId, GitError};
use thiserror::Error;

use crate::config::{Config, ConfigError};
use crate::exit_code::ExitCode;
use crate::host::LocalService;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TopicError {
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
    #[error("missing target to test the topic against")]
    MissingTarget,
    #[error("failed to synthesize merge request: {}", source)]
    CreateMergeRequest {
        #[from]
        source: HostingServiceError,
    },
    #[error("failed to load configuration: {}", source)]
    Config {
        #[from]
        source: ConfigError,
    },
    #[error("failed to run the checks: {}", source)]
    Check {
        #[from]
        source: check::CheckError,
    },
}

impl TopicError {
    fn invalid_configuration_file(path: PathBuf, source: io::Error) -> Self {
        TopicError::InvalidConfigurationFile {
            path,
            source,
        }
    }

    fn read_commit_configuration(commit: CommitId, source: GitError) -> Self {
        TopicError::ReadCommitConfiguration {
            commit,
            source,
        }
    }
}

type TopicResult<T> = Result<T, TopicError>;

pub struct Topic;

impl Topic {
    pub fn run(
        service: Arc<dyn LocalService>,
        matches: &ArgMatches,
        config: Option<&str>,
    ) -> TopicResult<ExitCode> {
        let local_service = service.clone();
        let service = service.as_hosting_service();

        let topic = if let Some(topic) = matches.value_of("TOPIC") {
            topic
        } else {
            "HEAD"
        };
        let target = matches
            .value_of("TARGET")
            .map(CommitId::new)
            .or_else(|| local_service.default_target())
            .ok_or(TopicError::MissingTarget)?;

        let ctx = local_service.git_context().clone();

        let conf_data = if let Some(config) = config {
            // Read the configuration from the file.
            Some(
                fs::read(config)
                    .map_err(|err| TopicError::invalid_configuration_file(config.into(), err))?,
            )
        } else {
            let config_commit = match matches.value_of("CONFIG_COMMIT") {
                Some("HEAD") => CommitId::new(topic),
                Some("TARGET") | None => target.clone(),
                Some(rev) => CommitId::new(rev),
            };

            // Read the configuration from the target branch.
            local_service
                .config(&config_commit)
                .map_err(|err| TopicError::read_commit_configuration(target.clone(), err))?
        };
        let conf;
        let check_conf = if let Some(data) = conf_data {
            conf = Config::from_bytes(data)?;
            conf.checks.check_configuration()
        } else {
            GitCheckConfiguration::new()
        };

        let check = check::Check::new(ctx, service, check_conf, &[]);
        let head = CommitId::new(topic);
        let mr = local_service.synth_merge_request(&head, &target)?;
        let status = check.check_mr("gf", &target, &mr)?;

        Ok(match status {
            check::CheckStatus::Pass => ExitCode::Success,
            check::CheckStatus::Fail => ExitCode::Failure,
        })
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("list")
            .about("check topic")
            .arg(
                Arg::new("TARGET")
                    .short('t')
                    .long("target")
                    .help("Target branch for integration")
                    .takes_value(true),
            )
            .arg(
                Arg::new("CONFIG_COMMIT")
                    .short('f')
                    .long("config-from")
                    .help("The commit to read configuration from (`HEAD`, `TARGET`, or revision)")
                    .takes_value(true),
            )
            .arg(Arg::new("TOPIC").help("Topic to check").takes_value(true))
    }
}
