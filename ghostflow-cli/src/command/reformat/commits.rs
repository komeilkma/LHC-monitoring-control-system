use std::sync::Arc;

use clap::{Arg, ArgMatches, Command};
use ghostflow::actions::reformat::{Reformat, ReformatError};
use ghostflow::host::{HostedProject, HostingServiceError};
use git_workarea::CommitId;
use thiserror::Error;

use crate::command::reformat;
use crate::host::LocalService;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CommitsError {
    #[error("missing target to reformat the topic against")]
    MissingTarget,
    #[error("hosting service error: {}", source)]
    Service {
        #[from]
        source: HostingServiceError,
    },
    #[error("reformatter action error: {}", source)]
    ReformatterAction {
        #[from]
        source: ReformatError,
    },
    #[error("reformatter command error: {}", source)]
    ReformatterCommand {
        #[from]
        source: Box<reformat::ReformatError>,
    },
}

impl From<reformat::ReformatError> for CommitsError {
    fn from(err: reformat::ReformatError) -> Self {
        Box::new(err).into()
    }
}

type CommitsResult<T> = Result<T, CommitsError>;

pub struct Commits;

impl Commits {
    pub fn run(
        service: Arc<dyn LocalService>,
        matches: &ArgMatches,
    ) -> CommitsResult<(CommitId, CommitId)> {
        let local_service = service.clone();
        let service = service.as_hosting_service();

        let topic = CommitId::new(matches.value_of("TOPIC").unwrap_or("HEAD"));
        let base = matches
            .value_of("BASE")
            .map(CommitId::new)
            .or_else(|| local_service.default_target())
            .ok_or(CommitsError::MissingTarget)?;
        let mr = local_service.synth_merge_request(&base, &topic)?;

        let project = HostedProject {
            name: mr.target_repo.name.clone(),
            service,
        };
        let ctx = local_service.git_context();
        let mut reformat = Reformat::new(ctx.clone(), project);

        reformat::config_from_args(local_service.as_ref(), &topic, matches).and_then(|conf| {
            Ok(conf
                .map(|conf| conf.add_formatters(&mut reformat))
                .transpose()?)
        })?;

        let reformatted_commit = reformat.reformat_mr(&base, &mr)?;

        Ok((topic, reformatted_commit))
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("commits")
            .about("reformat a range of commits")
            .arg(
                Arg::new("CONFIG")
                    .short('c')
                    .long("config")
                    .help("The configuration file to use")
                    .takes_value(true),
            )
            .arg(
                Arg::new("CONFIG_COMMIT")
                    .short('f')
                    .long("config-from")
                    .help("The commit to read configuration from (`HEAD` or revision)")
                    .conflicts_with("CONFIG")
                    .takes_value(true),
            )
            .arg(
                Arg::new("BASE")
                    .help("Newest commit to keep as-is")
                    .takes_value(true),
            )
            .arg(
                Arg::new("TOPIC")
                    .help("The head of the topic to reformat")
                    .takes_value(true),
            )
    }
}
