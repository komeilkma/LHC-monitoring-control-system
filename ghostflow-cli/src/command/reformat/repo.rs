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
pub enum RepoError {
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

impl From<reformat::ReformatError> for RepoError {
    fn from(err: reformat::ReformatError) -> Self {
        Box::new(err).into()
    }
}

type RepoResult<T> = Result<T, RepoError>;

pub struct Repo;

impl Repo {
    pub fn run(
        service: Arc<dyn LocalService>,
        matches: &ArgMatches,
    ) -> RepoResult<(CommitId, CommitId)> {
        let local_service = service.clone();
        let service = service.as_hosting_service();

        let commit = CommitId::new(
            matches
                .value_of("COMMIT")
                .expect("the COMMIT arg is required, yet missing?"),
        );
        let mr = local_service.synth_merge_request(&commit, &commit)?;

        let project = HostedProject {
            name: mr.target_repo.name.clone(),
            service,
        };
        let ctx = local_service.git_context();
        let mut reformat = Reformat::new(ctx.clone(), project);

        reformat::config_from_args(local_service.as_ref(), &commit, matches).and_then(|conf| {
            Ok(conf
                .map(|conf| conf.add_formatters(&mut reformat))
                .transpose()?)
        })?;

        let reformatted_commit = reformat.reformat_repo(&mr)?;

        Ok((commit, reformatted_commit))
    }

    pub fn subcommand() -> Command<'static> {
        Command::new("repo")
            .about("reformat the entire repository using an existing commit")
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
                Arg::new("COMMIT")
                    .help("Commit to use to reformat the entire repository")
                    .required(true)
                    .takes_value(true),
            )
    }
}
