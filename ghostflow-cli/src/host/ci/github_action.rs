use std::env::{self, VarError};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::sync::Arc;

use ghostflow::host::*;
use ghostflow_github::{Github, GithubError, GithubService};
use git_workarea::{CommitId, GitContext};
use thiserror::Error;

use crate::host::LocalService;

mod events {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct BranchAttrs {
        pub sha: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct PullRequestAttrs {
        pub number: u64,
        pub base: BranchAttrs,
    }

    #[derive(Debug, Deserialize)]
    pub struct PullRequest {
        pub pull_request: PullRequestAttrs,
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GithubActionError {
    #[error("authentication failure: {}", source)]
    Auth {
        #[source]
        source: GithubError,
    },
    #[error("service initialization failure: {}", source)]
    ServiceInit {
        #[source]
        source: Box<HostingServiceError>,
    },
    #[error("service data initialization failure: {}", source)]
    ServiceDataInit {
        #[source]
        source: Box<HostingServiceError>,
    },
    #[error("`GITHUB_EVENT_NAME` not found; is this running as a Github Action?")]
    NoEventName,
    #[error("the event name name must be valid Unicode; got {:?}", _0)]
    InvalidEventName(OsString),
    #[error("`GITHUB_EVENT_PATH` not found; is this running as a Github Action?")]
    NoEventPath,
    #[error("`GITHUB_REPOSITORY` not found; is this running as a Github Action?")]
    NoRepository,
    #[error("the repository name must be valid Unicode; got {:?}", _0)]
    InvalidProject(OsString),
    #[error("`GITHUB_WORKSPACE` not found; is this running as a Github Action?")]
    NoWorkspace,
    #[error("merge request required for a non-pull-request event")]
    NotPullRequest,
    #[error("failed to open the event data: {}", source)]
    OpenEventPath {
        #[source]
        source: io::Error,
    },
    #[error("failed to parse the {} event: {}", event, source)]
    ParseEvent {
        event: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

impl GithubActionError {
    fn auth(source: GithubError) -> Self {
        GithubActionError::Auth {
            source,
        }
    }

    fn service_init(source: HostingServiceError) -> Self {
        GithubActionError::ServiceInit {
            source: Box::new(source),
        }
    }

    fn service_data_init(source: HostingServiceError) -> Self {
        GithubActionError::ServiceDataInit {
            source: Box::new(source),
        }
    }

    fn open_event_path(source: io::Error) -> Self {
        GithubActionError::OpenEventPath {
            source,
        }
    }

    fn parse_event(event: &'static str, source: serde_json::Error) -> Self {
        GithubActionError::ParseEvent {
            event,
            source,
        }
    }
}

impl From<GithubActionError> for HostingServiceError {
    fn from(github: GithubActionError) -> Self {
        HostingServiceError::service(github)
    }
}

type GithubActionResult<T> = Result<T, GithubActionError>;

pub struct PullRequestInfo {
    id: u64,
    target: CommitId,
}

pub struct GithubAction {
    context: GitContext,
    service: GithubService,
    project: String,
    pr_info: Option<PullRequestInfo>,
}

impl GithubAction {
    pub fn new() -> GithubActionResult<Self> {
        const PULL_REQUEST: &str = "pull_request";

        let pr_info = match env::var("GITHUB_EVENT_NAME")
            .as_ref()
            .map(|name| name.as_ref())
        {
            Ok("push") => None,
            Ok(PULL_REQUEST) => {
                let path =
                    env::var_os("GITHUB_EVENT_PATH").ok_or(GithubActionError::NoEventPath)?;
                let file = File::open(path).map_err(GithubActionError::open_event_path)?;
                let reader = BufReader::new(file);
                let event: events::PullRequest = serde_json::from_reader(reader)
                    .map_err(|err| GithubActionError::parse_event(PULL_REQUEST, err))?;
                let pr = event.pull_request;
                Some(PullRequestInfo {
                    id: pr.number,
                    target: CommitId::new(pr.base.sha),
                })
            },
            Ok(name) => {
                // error!("Unknown event name (`{}`); ignoring event-specific information", name),
                let path =
                    env::var_os("GITHUB_EVENT_PATH").ok_or(GithubActionError::NoEventPath)?;
                println!(
                    "event data ({}): {:?}",
                    name,
                    fs::read(path)
                        .as_ref()
                        .map(|data| data.as_ref())
                        .map(String::from_utf8_lossy),
                );
                None
            },
            Err(VarError::NotPresent) => return Err(GithubActionError::NoEventName),
            Err(VarError::NotUnicode(name)) => {
                return Err(GithubActionError::InvalidEventName(name.clone()))
            },
        };

        let project = match env::var("GITHUB_REPOSITORY") {
            Ok(project) => project,
            Err(VarError::NotPresent) => return Err(GithubActionError::NoRepository),
            Err(VarError::NotUnicode(name)) => return Err(GithubActionError::InvalidProject(name)),
        };

        let path = env::var_os("GITHUB_WORKSPACE").ok_or(GithubActionError::NoWorkspace)?;
        let context = GitContext::new(path);

        // FIXME: Github Enterprise self hosted? 
        let host = "github.com";
        let client = Github::new_action(host).map_err(GithubActionError::auth)?;
        let service = GithubService::new(client).map_err(GithubActionError::service_init)?;

        Ok(Self {
            context,
            service,
            project,
            pr_info,
        })
    }
}

impl HostingService for GithubAction {
    fn service_user(&self) -> &User {
        self.service.service_user()
    }

    fn user(&self, project: &str, user: &str) -> Result<User, HostingServiceError> {
        self.service.user(project, user)
    }

    fn commit(&self, project: &str, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        self.service.commit(project, commit)
    }

    fn merge_request(&self, project: &str, id: u64) -> Result<MergeRequest, HostingServiceError> {
        self.service.merge_request(project, id)
    }

    fn repo(&self, project: &str) -> Result<Repo, HostingServiceError> {
        self.service.repo(project)
    }

    fn get_mr_comments(&self, mr: &MergeRequest) -> Result<Vec<Comment>, HostingServiceError> {
        self.service.get_mr_comments(mr)
    }

    fn post_mr_comment(&self, mr: &MergeRequest, content: &str) -> Result<(), HostingServiceError> {
        self.service.post_mr_comment(mr, content)
    }

    fn get_commit_statuses(
        &self,
        commit: &Commit,
    ) -> Result<Vec<CommitStatus>, HostingServiceError> {
        self.service.get_commit_statuses(commit)
    }

    fn post_commit_status(&self, status: PendingCommitStatus) -> Result<(), HostingServiceError> {
        self.service.post_commit_status(status)
    }

    fn get_mr_awards(&self, mr: &MergeRequest) -> Result<Vec<Award>, HostingServiceError> {
        self.service.get_mr_awards(mr)
    }

    fn issues_closed_by_mr(&self, mr: &MergeRequest) -> Result<Vec<Issue>, HostingServiceError> {
        self.service.issues_closed_by_mr(mr)
    }

    fn add_issue_labels(&self, issue: &Issue, labels: &[&str]) -> Result<(), HostingServiceError> {
        self.service.add_issue_labels(issue, labels)
    }
}

impl LocalService for GithubAction {
    fn as_hosting_service(self: Arc<Self>) -> Arc<dyn HostingService> {
        self
    }

    fn git_context(&self) -> &GitContext {
        &self.context
    }

    fn synth_merge_request(
        &self,
        head: &CommitId,
        target: &CommitId,
    ) -> Result<MergeRequest, HostingServiceError> {
        let pr_id = self
            .pr_info
            .as_ref()
            .ok_or(GithubActionError::NotPullRequest)
            .map(|pr| pr.id)?;
        let mut mr = self
            .service
            .merge_request(&self.project, pr_id)
            .map_err(GithubActionError::service_data_init)?;

        mr.source_branch = head.as_str().into();
        mr.target_branch = target.as_str().into();
        mr.commit.refname = Some(head.as_str().into());
        mr.commit.id = head.clone();

        Ok(mr)
    }

    fn synth_commit(&self, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        Ok(self
            .service
            .commit(&self.project, commit)
            .map_err(GithubActionError::service_data_init)?)
    }

    fn default_target(&self) -> Option<CommitId> {
        self.pr_info.as_ref().map(|pr| pr.target.clone())
    }
}
