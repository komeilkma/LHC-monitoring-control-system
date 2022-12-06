use clap::{Arg, ArgMatches, Command};
use git_checks_config::{BranchCheckConfig, CommitCheckConfig, TopicCheckConfig};
use itertools::Itertools;
use thiserror::Error;
use crate::exit_code::ExitCode;
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ListError {
    #[error("unknown check list format `{}`", format)]
    UnknownFormat { format: String },
}

impl ListError {
    fn unknown_format(format: String) -> Self {
        ListError::UnknownFormat {
            format,
        }
    }
}

type ListResult<T> = Result<T, ListError>;

pub struct List;

enum Format {
    Human,
}

impl Format {
    fn section(&self, section: &str) {
        match *self {
            Format::Human => {
                println!("{}:", section);
            },
        }
    }

    fn check(&self, name: &str) {
        match *self {
            Format::Human => {
                println!("\t{}", name);
            },
        }
    }
}

impl List {
    pub fn run(matches: &ArgMatches) -> ListResult<ExitCode> {
        let format = match matches.value_of("FORMAT").unwrap() {
            "human" => Format::Human,
            other => return Err(ListError::unknown_format(other.into())),
        };

        let (commit, branch, topic) = (
            matches.is_present("COMMIT"),
            matches.is_present("BRANCH"),
            matches.is_present("TOPIC"),
        );
        let all = !commit && !branch && !topic;

        if all || commit {
            format.section("commit checks");
            git_checks_config::inventory::iter::<CommitCheckConfig>
                .into_iter()
                .map(CommitCheckConfig::name)
                .sorted()
                .for_each(|name| format.check(name));
        }

        if all || branch {
            format.section("branch checks");
            git_checks_config::inventory::iter::<BranchCheckConfig>
                .into_iter()
                .map(BranchCheckConfig::name)
                .sorted()
                .for_each(|name| format.check(name));
        }

        if all || topic {
            format.section("topic checks");
            git_checks_config::inventory::iter::<TopicCheckConfig>
                .into_iter()
                .map(TopicCheckConfig::name)
                .sorted()
                .for_each(|name| format.check(name));
        }

        Ok(ExitCode::Success)
    }


}
