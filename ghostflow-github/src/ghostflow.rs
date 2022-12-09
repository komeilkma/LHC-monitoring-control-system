#![allow(unreachable_code)]
#![allow(unused_variables)]

use std::fmt::{self, Debug};

use chrono::Utc;
use ghostflow::host::*;
use git_workarea::{CommitId, GitContext};
use graphql_client::GraphQLQuery;
use log::{error, warn};
use serde_json::json;
use thiserror::Error;

use crate::authorization::CurrentUser;
use crate::client::Github;
use crate::queries;

const WORK_IN_PROGRESS_PREFIXES: &[&str] = &["WIP", "wip"];

impl From<CurrentUser> for User {
    fn from(user: CurrentUser) -> Self {
        Self {
            handle: user.login,
            email: user.email,
            name: user.name,
        }
    }
}

macro_rules! impl_from_reaction_content {
    ($type:path) => {
        impl From<$type> for String {
            fn from(reaction: $type) -> Self {
                use $type::*;
                match reaction {
                    CONFUSED => "confused".into(),
                    EYES => "eyes".into(),
                    HEART => "heart".into(),
                    HOORAY => "hooray".into(),
                    LAUGH => "laugh".into(),
                    ROCKET => "rocket".into(),
                    THUMBS_DOWN => "-1".into(),
                    THUMBS_UP => "+1".into(),
                    Other(s) => s,
                }
            }
        }
    };
}

impl_from_reaction_content!(queries::pull_request_reactions::ReactionContent);

macro_rules! impl_from_user_info {
    ($type:path) => {
        impl From<$type> for User {
            fn from(user: $type) -> Self {
                let $type {
                    name,
                    login,
                    // email,
                } = user;

                Self {
                    name: name.unwrap_or_else(|| login.clone()),
                    // TODO(github-enterprise): What email to use here?
                    email: format!("{}@users.noreply.github.com", login),
                    handle: login,
                }
            }
        }
    };
}

impl_from_user_info!(queries::pull_request_comments::UserInfo);
impl_from_user_info!(queries::pull_request_reactions::UserInfo);
impl_from_user_info!(queries::user::UserInfo);

macro_rules! impl_from_author_info {
    ($type:path) => {
        impl From<$type> for User {
            fn from(author: $type) -> Self {
                use $type::*;
                match author {
                    Bot(bot) => {
                        Self {
                            handle: bot.login.clone(),
                            // TODO(github-enterprise): What email to use here?
                            email: format!("{}@users.noreply.github.com", bot.login),
                            name: bot.login,
                        }
                    },
                    Mannequin(mannequin) => {
                        let (login, email) = (mannequin.login, mannequin.email);

                        Self {
                            handle: login.clone(),
                            email: email
                                // TODO(github-enterprise): What email to use here?
                                .unwrap_or_else(|| {
                                    format!("{}@mannequin.noreply.github.com", login)
                                }),
                            name: login,
                        }
                    },
                    Organization(org) => {
                        // XXX(nll)
                        let login = org.login.clone();

                        Self {
                            name: org.name.unwrap_or_else(|| login.clone()),
                            // TODO(github-enterprise): What email to use here?
                            email: format!("{}@users.noreply.github.com", login),
                            handle: login,
                        }
                    },
                    User(user) => {
                        // XXX(nll)
                        let login = user.login.clone();

                        Self {
                            name: user.name.unwrap_or_else(|| login.clone()),
                            // TODO(github-enterprise): What email to use here?
                            email: format!("{}@users.noreply.github.com", login),
                            handle: login,
                        }
                    },
                    EnterpriseUserAccount(user) => {
                        // XXX(nll)
                        let login = user.login.clone();

                        Self {
                            name: user.name.unwrap_or_else(|| login.clone()),
                            // TODO(github-enterprise): What email to use here?
                            email: format!("{}@users.noreply.github.com", login),
                            handle: login,
                        }
                    },
                }
            }
        }
    };
}

impl_from_author_info!(queries::pull_request::PullRequestInfoAuthor);
impl_from_author_info!(queries::pull_request_comments::IssueCommentInfoAuthor);
impl_from_author_info!(queries::pull_request_comments::PullRequestReviewInfoAuthor);

macro_rules! impl_from_comment_info {
    ($type:path) => {
        impl From<$type> for Option<Comment> {
            fn from(comment: $type) -> Self {
                let $type {
                    id: comment_id,
                    author,
                    created_at,
                    content,
                } = comment;

                author.map(|author| {
                    Comment {
                        id: comment_id,
                        is_system: false,
                        is_branch_update: false,
                        created_at,
                        author: author.into(),
                        content,
                    }
                })
            }
        }
    };
}

impl_from_comment_info!(queries::pull_request_comments::IssueCommentInfo);
impl_from_comment_info!(queries::pull_request_comments::PullRequestReviewInfo);

impl From<queries::commit_statuses::CheckConclusionState> for CommitStatusState {
    fn from(state: queries::commit_statuses::CheckConclusionState) -> Self {
        use queries::commit_statuses::CheckConclusionState;
        match state {
            CheckConclusionState::ACTION_REQUIRED
            | CheckConclusionState::FAILURE
            | CheckConclusionState::STARTUP_FAILURE
            | CheckConclusionState::TIMED_OUT => CommitStatusState::Failed,
            CheckConclusionState::SKIPPED
            | CheckConclusionState::STALE
            | CheckConclusionState::CANCELLED => CommitStatusState::Pending,
            CheckConclusionState::NEUTRAL | CheckConclusionState::SUCCESS => {
                CommitStatusState::Success
            },
            CheckConclusionState::Other(s) => {
                error!(
                    target: "github",
                    "new GitHub conclusion state: {}",
                    s,
                );
                CommitStatusState::Failed
            },
        }
    }
}

/*
fn extract_run_state(
    state: CommitStatusState,
) -> (
    Option<queries::post_check_run::CheckConclusionState>,
    queries::post_check_run::RequestableCheckStatusState,
) {
    use queries::post_check_run::CheckConclusionState::*;
    use queries::post_check_run::RequestableCheckStatusState::*;
    match state {
        CommitStatusState::Pending => (None, QUEUED),
        CommitStatusState::Running => (None, IN_PROGRESS),
        CommitStatusState::Success => (Some(SUCCESS), COMPLETED),
        CommitStatusState::Failed => (Some(FAILURE), COMPLETED),
    }
}
*/

fn extract_run_state_rest(state: CommitStatusState) -> (Option<&'static str>, &'static str) {
    match state {
        CommitStatusState::Pending => (None, "queued"),
        CommitStatusState::Running => (None, "in_progress"),
        CommitStatusState::Success => (Some("success"), "completed"),
        CommitStatusState::Failed => (Some("failure"), "completed"),
    }
}

/// Structure used to communicate with a Github instance.
///
/// The API calls associated with this structure assume that the following permissions in GitHub
/// have been granted to the application:
///
///   - Read & write
///     * Checks
///     * Issues
///     * Pull requests
///   - Read-only
///     * Repository contents
///     * Repository metadata
///
/// User permissions should include read-only access to email addresses. Note that this does not
/// currently work however and even with that permission, reading email addresses is being denied.
pub struct GithubService {
    /// The Github client.
    github: Github,
    /// The user the service is acting as.
    user: User,
}

const GITHUB_CHECK_RUN_MESSAGE_LIMIT: usize = 65535;
const GITHUB_OVERFLOW_INDICATOR: &str = "**Contents exceed GitHub check limits**\n\n";

fn trim_to_check_run_limit(text: String) -> String {
    if text.len() > GITHUB_CHECK_RUN_MESSAGE_LIMIT {
        warn!(
            target: "github",
            "Check results comment exceeding limits: {}",
            text,
        );

        format!(
            "{}{}",
            // Prepend because we have no idea what the Markdown parser state will be where we
            // truncate the intended message.
            GITHUB_OVERFLOW_INDICATOR,
            &text[..GITHUB_CHECK_RUN_MESSAGE_LIMIT - GITHUB_OVERFLOW_INDICATOR.len()],
        )
    } else {
        text
    }
}

impl GithubService {
    /// Create a new Github communication channel.
    pub fn new(github: Github) -> Result<Self, HostingServiceError> {
        let user = github.current_user().map_err(HostingServiceError::host)?;

        Ok(Self {
            user: user.into(),
            github,
        })
    }

    /// Splits a project name in to an owner, name pair.
    pub fn split_project(project: &str) -> Result<(&str, &str), HostingServiceError> {
        let mut split = project.split('/');
        if let Some(owner) = split.next() {
            if let Some(name) = split.next() {
                Ok((owner, name))
            } else {
                Err(GithubServiceError::missing_repository(project.into()).into())
            }
        } else {
            Err(GithubServiceError::missing_owner(project.into()).into())
        }
    }

    /// Create a repository from a Github project.
    fn repo<R>(&self, project: R) -> Result<Repo, HostingServiceError>
    where
        R: queries::RepoInfo,
    {
        self.repo_impl(project.name(), project.ssh_url(), project.parent())
    }

    /// Create a repository from a Github project.
    fn repo_impl(
        &self,
        name: String,
        ssh_url: &str,
        parent: Option<queries::RepoParentInfo>,
    ) -> Result<Repo, HostingServiceError> {
        let parent_project = if let Some(parent_info) = parent {
            let queries::RepoParentInfo {
                owner,
                name,
                ssh_url,
                parent: grand_parent,
            } = parent_info;

            let grand_parent = if let Some((owner, name)) = grand_parent {
                let vars = queries::repository::Variables {
                    owner: owner.into(),
                    name: name.into(),
                };
                let query = queries::Repository::build_query(vars);
                let grand_parent_project = self
                    .github
                    .send::<queries::Repository>(owner, &query)
                    .map_err(HostingServiceError::host)
                    .and_then(|rsp| {
                        Ok(rsp
                            .repository
                            .ok_or_else(|| GithubHostError::no_repository(name.into()))?)
                    })?;
                Some(Box::new(self.repo(grand_parent_project)?))
            } else {
                None
            };

            Some(Box::new(Repo {
                name: format!("{}/{}", owner, name),
                url: ssh_url.into(),
                forked_from: grand_parent,
            }))
        } else {
            None
        };

        Ok(Repo {
            name,
            url: ssh_url.into(),
            forked_from: parent_project,
        })
    }

    /// Create a comment.
    pub fn post_comment<C>(
        &self,
        owner: &str,
        id: String,
        content: C,
    ) -> Result<(), HostingServiceError>
    where
        C: Into<String>,
    {
        let input = queries::post_comment::Variables {
            input: queries::post_comment::AddCommentInput {
                // TODO: Make a mutation ID.
                client_mutation_id: None,
                subject_id: id,
                body: content.into(),
            },
        };
        let mutation = queries::PostComment::build_query(input);
        self.github
            .send::<queries::PostComment>(owner, &mutation)
            .map_err(HostingServiceError::host)?;

        Ok(())
    }

    /// Create a check run.
    fn post_check_run(
        &self,
        status: PendingCommitStatus,
        description: Option<String>,
    ) -> Result<(), HostingServiceError> {
        let project = &status.commit.repo.name;
        let (owner, name) = Self::split_project(project)?;

        let endpoint = format!("repos/{}/{}/check-runs", owner, name);
        let (conclusion, status_state) = extract_run_state_rest(status.state);
        let output = if let Some(description) = description {
            let description = trim_to_check_run_limit(description);

            json!({
                "title": status.name,
                "summary": status.description,
                "text": description,
            })
        } else {
            json!({
                "title": status.name,
                "summary": status.description,
            })
        };
        let mut data = if let Some(conclusion) = conclusion {
            json!({
                "name": status.name,
                "head_sha": status.commit.id.as_str(),
                "status": status_state,
                "conclusion": conclusion,
                "completed_at": Utc::now(),
                "output": output,
            })
        } else {
            json!({
                "name": status.name,
                "head_sha": status.commit.id.as_str(),
                "status": status_state,
                "output": output,
            })
        };
        if let Some(target_url) = status.target_url {
            data.as_object_mut()
                .expect("`data` is always constructed as an object")
                .insert("details_url".into(), target_url.into());
        }
        self.github
            .post(owner, &endpoint, &data)
            .map_err(HostingServiceError::host)?;

        /*
        let vars = queries::repository_id::Variables {
            owner: owner.into(),
            name: name.into(),
        };
        let query = queries::RepositoryID::build_query(vars);
        let repository_id = self.github
            .send::<queries::RepositoryID>(owner, &query)
            .compat()
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(&rsp.rate_limit_info.rate_limit, queries::RepositoryID::name());
                Ok(rsp.repository
                    .ok_or_else(|| GithubHostError::no_repository(project))?)
            })?
            .id;

        let (conclusion, status_state) = extract_run_state(status.state);
        let input = queries::post_check_run::Variables {
            input: queries::post_check_run::CreateCheckRunInput {
                // TODO: Make a mutation ID.
                client_mutation_id: None,
                actions: None,
                completed_at: conclusion.map(|_| Utc::now()),
                conclusion: conclusion,
                details_url: status.target_url.map(Into::into),
                external_id: None,
                head_sha: status.commit.id.as_str().into(),
                name: status.name.into(),
                output: Some(queries::post_check_run::CheckRunOutput {
                    annotations: None,
                    images: None,
                    summary: status.description.into(),
                    text: description,
                    title: status.name.into(),
                }),
                repository_id: repository_id,
                started_at: None,
                status: Some(status_state),
            },
        };
        let mutation = queries::PostCheckRun::build_query(input);
        self.github
            .send::<queries::PostCheckRun>(owner, &mutation)
            .compat()
            .map_err(HostingServiceError::host)?;
        */

        Ok(())
    }

    /// Check the rate limiting for a query.
    pub fn check_rate_limits<R>(rate_limit: &Option<R>, name: &str)
    where
        R: Into<queries::RateLimitInfo> + Clone,
    {
        if let Some(info) = rate_limit.as_ref() {
            info.clone().into().inspect(name);
        }
    }

    /// Access the GitHub client.
    pub fn github(&self) -> &Github {
        &self.github
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
enum GithubHostError {
    #[error("no repository named {}", project)]
    NoRepository { project: String },
    #[error("no user named {}", user)]
    NoUser { user: String },
    #[error("no such object {}@{}", project, object)]
    NoObject { object: CommitId, project: String },
    #[error("{}@{} is not a commit", project, object)]
    NotCommit { object: CommitId, project: String },
    #[error("no such issue {}#{}", project, issue)]
    NoIssue { issue: u64, project: String },
    #[error("no such pull {}#{}", project, pull)]
    NoPull { pull: u64, project: String },
    #[error("no author for pull {}#{}", project, pull)]
    NoPullAuthor { pull: u64, project: String },
    #[error("no label {} in {}", label, project)]
    NoLabel { label: String, project: String },
    #[error("no pull timeline edges on {}#{}", project, pull)]
    NoPullTimelineEdges { pull: u64, project: String },
    #[error("no pull reaction edges on {}#{}", project, pull)]
    NoPullReactionEdges { pull: u64, project: String },
    #[error("no issues closed by pr edges on {}#{}", project, pull)]
    NoIssuesClosedByPrEdges { pull: u64, project: String },
    #[error("no closing issues found on pr {}#{}", project, pull)]
    NoClosingIssues { pull: u64, project: String },
}

impl GithubHostError {
    fn no_repository(project: String) -> Self {
        GithubHostError::NoRepository {
            project,
        }
    }

    fn no_user(user: String) -> Self {
        GithubHostError::NoUser {
            user,
        }
    }

    fn no_object(object: CommitId, project: String) -> Self {
        GithubHostError::NoObject {
            object,
            project,
        }
    }

    fn not_commit(object: CommitId, project: String) -> Self {
        GithubHostError::NotCommit {
            object,
            project,
        }
    }

    fn no_issue(issue: u64, project: String) -> Self {
        GithubHostError::NoIssue {
            issue,
            project,
        }
    }

    fn no_pull(pull: u64, project: String) -> Self {
        GithubHostError::NoPull {
            pull,
            project,
        }
    }

    fn no_pull_author(pull: u64, project: String) -> Self {
        GithubHostError::NoPullAuthor {
            pull,
            project,
        }
    }

    fn no_label(label: String, project: String) -> Self {
        GithubHostError::NoLabel {
            label,
            project,
        }
    }

    fn no_pull_timeline_edges(pull: u64, project: String) -> Self {
        GithubHostError::NoPullTimelineEdges {
            pull,
            project,
        }
    }

    fn no_pull_reaction_edges(pull: u64, project: String) -> Self {
        GithubHostError::NoPullReactionEdges {
            pull,
            project,
        }
    }

    fn no_issues_closed_by_pr_edges(pull: u64, project: String) -> Self {
        GithubHostError::NoIssuesClosedByPrEdges {
            pull,
            project,
        }
    }

    fn no_closing_issues(pull: u64, project: String) -> Self {
        GithubHostError::NoClosingIssues {
            pull,
            project,
        }
    }
}

impl From<GithubHostError> for HostingServiceError {
    fn from(github: GithubHostError) -> Self {
        HostingServiceError::host(github)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
enum GithubServiceError {
    #[error("missing repository name in {}", project)]
    MissingRepository { project: String },
    #[error("missing owner name in {}", project)]
    MissingOwner { project: String },
}

impl GithubServiceError {
    fn missing_repository(project: String) -> Self {
        GithubServiceError::MissingRepository {
            project,
        }
    }

    fn missing_owner(project: String) -> Self {
        GithubServiceError::MissingOwner {
            project,
        }
    }
}

impl From<GithubServiceError> for HostingServiceError {
    fn from(github: GithubServiceError) -> Self {
        HostingServiceError::service(github)
    }
}

impl HostingService for GithubService {
    fn fetch_mr(&self, git: &GitContext, mr: &MergeRequest) -> Result<(), HostingServiceError> {
        git.fetch(&mr.target_repo.url, [&format!("refs/pull/{}/head", mr.id)])
            .map_err(HostingServiceError::fetch)
    }

    fn service_user(&self) -> &User {
        &self.user
    }

    fn user(&self, project: &str, user: &str) -> Result<User, HostingServiceError> {
        let (owner, _) = Self::split_project(project)?;

        // XXX(github): Querying for bots by username has no endpoint at the moment. Just
        // synthesize the data.
        if user.ends_with("[bot]") {
            return Ok(User {
                handle: user.into(),
                name: user.into(),
                // TODO(github-enterprise): What email to use here?
                email: format!("{}@users.noreply.github.com", user),
            });
        }

        let vars = queries::user::Variables {
            name: user.into(),
        };
        let query = queries::User::build_query(vars);
        Ok(self
            .github
            .send::<queries::User>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(&rsp.rate_limit_info.rate_limit, queries::User::name());
                Ok(rsp
                    .user
                    .ok_or_else(|| GithubHostError::no_user(user.into()))?)
            })?
            .into())
    }

    fn commit(&self, project: &str, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::commit::Variables {
            owner: owner.into(),
            name: name.into(),
            commit: commit.as_str().into(),
        };
        let query = queries::Commit::build_query(vars);
        self.github
            .send::<queries::Commit>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(&rsp.rate_limit_info.rate_limit, queries::Commit::name());
                Ok(rsp
                    .repository
                    .ok_or_else(|| GithubHostError::no_repository(project.into()))?)
            })
            .and_then(|repo| {
                Ok(repo
                    .object
                    .ok_or_else(|| GithubHostError::no_object(commit.clone(), project.into()))?)
            })
            .and_then(|object| {
                let (repo, oid, object) = (object.repository, object.oid, object.on);

                use queries::commit::CommitRepositoryObjectOn;
                let oid = if let CommitRepositoryObjectOn::Commit = object {
                    oid
                } else {
                    return Err(GithubHostError::not_commit(commit.clone(), project.into()).into());
                };

                Ok(Commit {
                    repo: self.repo(repo)?,
                    refname: None,
                    id: CommitId::new(oid),
                    // Github does have a "databaseId" for a given check suite, but does not
                    // expose a way to *query* on it.
                    last_pipeline: None,
                })
            })
    }

    fn merge_request(&self, project: &str, id: u64) -> Result<MergeRequest, HostingServiceError> {
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::pull_request::Variables {
            owner: owner.into(),
            name: name.into(),
            pull: id as i64,
        };
        let query = queries::PullRequest::build_query(vars);
        self.github
            .send::<queries::PullRequest>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(
                    &rsp.rate_limit_info.rate_limit,
                    queries::PullRequest::name(),
                );
                Ok(rsp
                    .repository
                    .ok_or_else(|| GithubHostError::no_repository(project.into()))?)
            })
            .and_then(|repo| {
                Ok(repo
                    .pull_request
                    .ok_or_else(|| GithubHostError::no_pull(id, project.into()))?)
            })
            .and_then(|pull| {
                let queries::pull_request::PullRequestInfo {
                    source_repo,
                    source_branch,
                    target_repo,
                    target_branch,
                    url,
                    title,
                    description,
                    head_ref_oid,
                    author,
                    is_draft,
                } = pull;

                let target_repo = self.repo(target_repo)?;

                Ok(MergeRequest {
                    // TODO(github): Is this `None` if the source repo is also the target repo?
                    // There is an `isCrossRepository` flag on pull requests.
                    source_repo: if let Some(repo) = source_repo {
                        Some(self.repo(repo)?)
                    } else {
                        None
                    },
                    source_branch: source_branch.clone(),
                    target_repo: target_repo.clone(),
                    target_branch,
                    id,
                    url,
                    work_in_progress: is_draft
                        || WORK_IN_PROGRESS_PREFIXES
                            .iter()
                            .any(|prefix| title.starts_with(prefix)),
                    description,
                    old_commit: None,
                    commit: Commit {
                        repo: target_repo,
                        refname: Some(source_branch),
                        id: CommitId::new(head_ref_oid),
                        // Github does have a "databaseId" for a given check suite, but does not
                        // expose a way to *query* on it.
                        last_pipeline: None,
                    },
                    author: author
                        .ok_or_else(|| GithubHostError::no_pull_author(id, project.into()))?
                        .into(),
                    reference: format!("#{}", id),
                    remove_source_branch: false,
                })
            })
    }

    fn repo(&self, project: &str) -> Result<Repo, HostingServiceError> {
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::repository::Variables {
            owner: owner.into(),
            name: name.into(),
        };
        let query = queries::Repository::build_query(vars);
        self.repo(
            self.github
                .send::<queries::Repository>(owner, &query)
                .map_err(HostingServiceError::host)
                .and_then(|rsp| {
                    Self::check_rate_limits(
                        &rsp.rate_limit_info.rate_limit,
                        queries::Repository::name(),
                    );
                    Ok(rsp
                        .repository
                        .ok_or_else(|| GithubHostError::no_repository(project.into()))?)
                })?,
        )
    }

    fn get_mr_comments(&self, mr: &MergeRequest) -> Result<Vec<Comment>, HostingServiceError> {
        let project = &mr.target_repo.name;
        let id = mr.id;
        let (owner, name) = Self::split_project(project)?;

        let mut vars = queries::pull_request_comments::Variables {
            owner: owner.into(),
            name: name.into(),
            pull: id as i64,
            cursor: None,
        };

        let mut comments = Vec::new();
        loop {
            let query = queries::PullRequestComments::build_query(vars.clone());
            let page_timeline = self
                .github
                .send::<queries::PullRequestComments>(owner, &query)
                .map_err(HostingServiceError::host)
                .and_then(|rsp| {
                    Self::check_rate_limits(
                        &rsp.rate_limit_info.rate_limit,
                        queries::PullRequestComments::name(),
                    );
                    Ok(rsp
                        .repository
                        .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
                })
                .and_then(|rsp| {
                    Ok(rsp
                        .pull_request
                        .ok_or_else(|| GithubHostError::no_pull(id, project.clone()))?)
                })?
                .timeline_items;
            let (items, page_info) = (
                page_timeline
                    .items
                    .ok_or_else(|| GithubHostError::no_pull_timeline_edges(id, project.into()))?,
                page_timeline.page_info,
            );

            comments.extend(
                items.into_iter()
                    .filter_map(|item| {
                        use queries::pull_request_comments::PullRequestCommentsRepositoryPullRequestTimelineItemsItems::*;
                        match item {
                            Some(PullRequestCommit(pr_commit)) => {
                                let queries::pull_request_comments::CommitInfo {
                                    oid: commit_id,
                                    pushed_date,
                                    committed_date,
                                    author,
                                    message,
                                } = pr_commit.commit;

                                author.map(|author| {
                                    let queries::pull_request_comments::CommitInfoAuthor {
                                        user,
                                        name,
                                        email,
                                    } = author;

                                    Comment {
                                        id: commit_id,
                                        is_system: true,
                                        is_branch_update: pushed_date.is_some(),
                                        // This is a poor replacement for `pushedDate`, but it's
                                        // the best we have.
                                        created_at: pushed_date.unwrap_or(committed_date),
                                        author: user.map(|user| user.into())
                                            .unwrap_or_else(|| {
                                                // XXX(github): We can't really drop things just
                                                // because we don't have data since the glostflow
                                                // code expects to be able to find when the last
                                                // push happened and these "comments" fulfill that
                                                // use case. If anything is missing, just use who
                                                // we are communicating as.
                                                User {
                                                    name: name.unwrap_or_else(|| self.user.name.clone()),
                                                    email: email.unwrap_or_else(|| self.user.name.clone()),
                                                    handle: self.user.handle.clone(),
                                                }
                                            }),
                                        content: message,
                                    }
                                })
                            },
                            Some(IssueComment(comment)) => {
                                comment.into()
                            },
                            Some(PullRequestReview(review)) => {
                                review.into()
                            },
                            _ => None,
                        }
                    })
            );

            if page_info.has_next_page {
                // XXX: We are assuming that if `has_next_page` is `true` that we'll have an
                // `end_cursor`.
                assert!(
                    page_info.end_cursor.is_some(),
                    "GitHub gave us a new page without a cursor to follow.",
                );
                vars.cursor = page_info.end_cursor;
            } else {
                break;
            }
        }

        Ok(comments)
    }

    fn post_mr_comment(&self, mr: &MergeRequest, content: &str) -> Result<(), HostingServiceError> {
        let project = &mr.target_repo.name;
        let id = mr.id;
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::pull_request_id::Variables {
            owner: owner.into(),
            name: name.into(),
            pull: id as i64,
        };
        let query = queries::PullRequestID::build_query(vars);
        let pull_request_id = self
            .github
            .send::<queries::PullRequestID>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(
                    &rsp.rate_limit_info.rate_limit,
                    queries::PullRequestID::name(),
                );
                Ok(rsp
                    .repository
                    .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
            })
            .and_then(|rsp| {
                Ok(rsp
                    .pull_request
                    .ok_or_else(|| GithubHostError::no_pull(id, project.clone()))?)
            })?
            .id;

        self.post_comment(owner, pull_request_id, content)
    }

    fn get_commit_statuses(
        &self,
        commit: &Commit,
    ) -> Result<Vec<CommitStatus>, HostingServiceError> {
        let project = &commit.repo.name;
        let oid = commit.id.as_str();
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::commit_statuses::Variables {
            owner: owner.into(),
            name: name.into(),
            commit: oid.into(),
            app_id: self.github.app_id(),
        };

        let query = queries::CommitStatuses::build_query(vars);
        let check_suite = self
            .github
            .send::<queries::CommitStatuses>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(
                    &rsp.rate_limit_info.rate_limit,
                    queries::CommitStatuses::name(),
                );
                Ok(rsp
                    .repository
                    .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
            })
            .and_then(|rsp| {
                let object = rsp.object.ok_or_else(|| {
                    GithubHostError::no_object(CommitId::new(oid), project.clone())
                })?;

                use queries::commit_statuses::CommitStatusesRepositoryObject;
                if let CommitStatusesRepositoryObject::Commit(commit) = object {
                    Ok(commit)
                } else {
                    Err(GithubHostError::not_commit(CommitId::new(oid), project.clone()).into())
                }
            })?
            .check_suites
            .and_then(|check_suites| check_suites.check_suite)
            .unwrap_or_default()
            .into_iter()
            .next()
            .and_then(|check_suite| check_suite);
        let check_suite = if let Some(check_suite) = check_suite {
            check_suite
        } else {
            return Ok(Vec::new());
        };
        let (branch, check_runs) = (
            check_suite.branch.map(|branch| branch.name),
            check_suite
                .check_runs
                .and_then(|check_runs| check_runs.check_runs)
                .unwrap_or_default(),
        );

        Ok(check_runs.into_iter()
            .filter_map(|check_run| {
                check_run.and_then(|check_run| {
                    let queries::commit_statuses::CommitStatusesRepositoryObjectOnCommitCheckSuitesCheckSuiteCheckRunsCheckRuns {
                        conclusion,
                        name,
                        summary,
                        details_url,
                    } = check_run;

                    conclusion.map(|conclusion| {
                        CommitStatus {
                            state: conclusion.into(),
                            author: self.user.clone(),
                            refname: branch.clone(),
                            name,
                            description: summary.unwrap_or_default(),
                            target_url: details_url,
                        }
                    })
                })
            })
            .collect())
    }

    fn post_commit_status(&self, status: PendingCommitStatus) -> Result<(), HostingServiceError> {
        self.post_check_run(status, None)
    }

    fn post_review(
        &self,
        status: PendingCommitStatus,
        _: &MergeRequest,
        description: &str,
    ) -> Result<(), HostingServiceError> {
        self.post_check_run(status, Some(description.into()))
    }

    fn get_mr_awards(&self, mr: &MergeRequest) -> Result<Vec<Award>, HostingServiceError> {
        let project = &mr.target_repo.name;
        let id = mr.id;
        let (owner, name) = Self::split_project(project)?;

        let mut vars = queries::pull_request_reactions::Variables {
            owner: owner.into(),
            name: name.into(),
            pull: id as i64,
            cursor: None,
        };

        let mut awards = Vec::new();
        loop {
            let query = queries::PullRequestReactions::build_query(vars.clone());
            let page_reactions = self
                .github
                .send::<queries::PullRequestReactions>(owner, &query)
                .map_err(HostingServiceError::host)
                .and_then(|rsp| {
                    Self::check_rate_limits(
                        &rsp.rate_limit_info.rate_limit,
                        queries::PullRequestReactions::name(),
                    );
                    Ok(rsp
                        .repository
                        .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
                })
                .and_then(|rsp| {
                    Ok(rsp
                        .pull_request
                        .ok_or_else(|| GithubHostError::no_pull(id, project.clone()))?)
                })?
                .reactions;
            let (reactions, page_info) = (
                page_reactions
                    .reactions
                    .ok_or_else(|| GithubHostError::no_pull_reaction_edges(id, project.into()))?,
                page_reactions.page_info,
            );

            awards.extend(
                reactions.into_iter()
                    .filter_map(|reaction| {
                        reaction.and_then(|reaction| {
                            let queries::pull_request_reactions::PullRequestReactionsRepositoryPullRequestReactionsReactions {
                                content,
                                user,
                                ..
                            } = reaction;

                            user.map(|user| {
                                Award {
                                    name: content.into(),
                                    author: user.into(),
                                }
                            })
                        })
                    })
            );

            if page_info.has_next_page {
                // XXX: We are assuming that if `has_next_page` is `true` that we'll have an
                // `end_cursor`.
                assert!(
                    page_info.end_cursor.is_some(),
                    "GitHub gave us a new page without a cursor to follow.",
                );
                vars.cursor = page_info.end_cursor;
            } else {
                break;
            }
        }

        Ok(awards)
    }

    fn issues_closed_by_mr(&self, mr: &MergeRequest) -> Result<Vec<Issue>, HostingServiceError> {
        let project = &mr.target_repo.name;
        let id = mr.id;
        let (owner, name) = Self::split_project(project)?;

        let mut vars = queries::issues_closed_by_pull_request::Variables {
            owner: owner.into(),
            name: name.into(),
            pull: id as i64,
            cursor: None,
        };

        let mut issues: Vec<Issue> = Vec::new();
        loop {
            let query = queries::IssuesClosedByPullRequest::build_query(vars.clone());
            let page_issues = self
                .github
                .send::<queries::IssuesClosedByPullRequest>(owner, &query)
                .map_err(HostingServiceError::host)
                .and_then(|rsp| {
                    Self::check_rate_limits(
                        &rsp.rate_limit_info.rate_limit,
                        queries::IssuesClosedByPullRequest::name(),
                    );
                    Ok(rsp
                        .repository
                        .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
                })
                .and_then(|rsp| {
                    Ok(rsp
                        .pull_request
                        .ok_or_else(|| GithubHostError::no_pull(id, project.clone()))?)
                })
                .and_then(|rsp| {
                    Ok(rsp
                        .closing_issues_references
                        .ok_or_else(|| GithubHostError::no_closing_issues(id, project.clone()))?)
                })?;
            let (page_issues, page_info) = (
                page_issues.issues.ok_or_else(|| {
                    GithubHostError::no_issues_closed_by_pr_edges(id, project.into())
                })?,
                page_issues.page_info,
            );

            issues.extend(
                page_issues.into_iter()
                    .filter_map(|issue| {
                        issue.and_then(|issue| {
                            let queries::issues_closed_by_pull_request::IssuesClosedByPullRequestRepositoryPullRequestClosingIssuesReferencesIssues {
                                repository,
                                number,
                                url,
                                labels,
                                ..
                            } = issue;

                            let repo = self.repo(repository).ok()?;
                            let id = if number < 0 {
                                0
                            } else {
                                number as u64
                            };
                            let labels = labels
                                .and_then(|labels| labels.names)
                                .map(|names| {
                                    names.into_iter()
                                        .filter_map(|label| label.map(|label| label.name))
                                        .collect()
                                })
                                .unwrap_or_else(Vec::new);

                            Some(Issue {
                                reference: format!("{}#{}", repo.name, number),
                                repo,
                                id,
                                url,
                                labels,
                            })
                        })
                    })
            );

            if page_info.has_next_page {
                // XXX: We are assuming that if `has_next_page` is `true` that we'll have an
                // `end_cursor`.
                assert!(
                    page_info.end_cursor.is_some(),
                    "GitHub gave us a new page without a cursor to follow.",
                );
                vars.cursor = page_info.end_cursor;
            } else {
                break;
            }
        }

        Ok(issues)
    }

    fn add_issue_labels(&self, issue: &Issue, labels: &[&str]) -> Result<(), HostingServiceError> {
        let project = &issue.repo.name;
        let id = issue.id;
        let (owner, name) = Self::split_project(project)?;

        let vars = queries::issue_id::Variables {
            owner: owner.into(),
            name: name.into(),
            issue: id as i64,
        };
        let query = queries::IssueID::build_query(vars);
        let issue_id = self
            .github
            .send::<queries::IssueID>(owner, &query)
            .map_err(HostingServiceError::host)
            .and_then(|rsp| {
                Self::check_rate_limits(&rsp.rate_limit_info.rate_limit, queries::IssueID::name());
                Ok(rsp
                    .repository
                    .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
            })
            .and_then(|rsp| {
                Ok(rsp
                    .issue
                    .ok_or_else(|| GithubHostError::no_issue(id, project.clone()))?)
            })?
            .id;

        let label_ids = labels
            .iter()
            .map(|&label| {
                let vars = queries::label_id::Variables {
                    owner: owner.into(),
                    name: name.into(),
                    label: label.into(),
                };
                let query = queries::LabelID::build_query(vars);
                Ok(self
                    .github
                    .send::<queries::LabelID>(owner, &query)
                    .map_err(HostingServiceError::host)
                    .and_then(|rsp| {
                        Self::check_rate_limits(
                            &rsp.rate_limit_info.rate_limit,
                            queries::LabelID::name(),
                        );
                        Ok(rsp
                            .repository
                            .ok_or_else(|| GithubHostError::no_repository(project.clone()))?)
                    })
                    .and_then(|rsp| {
                        Ok(rsp.label.ok_or_else(|| {
                            GithubHostError::no_label(label.into(), project.clone())
                        })?)
                    })?
                    .id)
            })
            .collect::<Result<_, HostingServiceError>>()?;

        let input = queries::add_issue_labels::Variables {
            input: queries::add_issue_labels::AddLabelsToLabelableInput {
                // TODO: Make a mutation ID.
                client_mutation_id: None,
                label_ids,
                labelable_id: issue_id,
            },
        };
        let mutation = queries::AddIssueLabels::build_query(input);
        self.github
            .send::<queries::AddIssueLabels>(owner, &mutation)
            .map_err(HostingServiceError::host)?;

        Ok(())
    }
}

impl Debug for GithubService {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GithubService")
            .field("user", &self.user.handle)
            .finish()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_github_trim() {
        use super::{
            trim_to_check_run_limit, GITHUB_CHECK_RUN_MESSAGE_LIMIT, GITHUB_OVERFLOW_INDICATOR,
        };

        let just_short_enough = format!("{:width$}", 0, width = GITHUB_CHECK_RUN_MESSAGE_LIMIT);
        assert_eq!(just_short_enough.len(), GITHUB_CHECK_RUN_MESSAGE_LIMIT);
        let long_text = format!("{:width$}", 0, width = GITHUB_CHECK_RUN_MESSAGE_LIMIT + 1);
        assert!(long_text.len() > GITHUB_CHECK_RUN_MESSAGE_LIMIT);
        let long_text_trimmed = format!(
            "{}{:width$}",
            GITHUB_OVERFLOW_INDICATOR,
            "", // the `0` will be overflowed out, so fill with blanks.
            width = GITHUB_CHECK_RUN_MESSAGE_LIMIT - GITHUB_OVERFLOW_INDICATOR.len(),
        );
        assert_eq!(long_text_trimmed.len(), GITHUB_CHECK_RUN_MESSAGE_LIMIT);

        let cases = [
            ("", ""),
            ("short", "short"),
            (&just_short_enough, &just_short_enough),
            (&long_text, &long_text_trimmed),
        ];

        for (input, expected) in cases {
            let actual = trim_to_check_run_limit(input.into());
            assert!(actual.len() <= GITHUB_CHECK_RUN_MESSAGE_LIMIT);
            assert_eq!(actual, expected);
        }
    }
}
