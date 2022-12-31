use std::collections::hash_map::HashMap;
use std::fmt;
use std::sync::Arc;
use ghostflow::host::*;
use git_workarea::CommitId;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use serde::de::DeserializeOwned;
use thiserror::Error;

pub use gitlab;

use gitlab::api::{self, Query};

mod types;

lazy_static! {
    static ref MR_UPDATE_RE: Regex = Regex::new(
        "^[Aa]dded [0-9][0-9]* (new )?commits?:?\n\
         (\n<ul>(<li>[0-9a-f.]+ - .*?</li>)*</ul>|(\n\\* [0-9a-f.]+ - [^\n]*)*)\
         (\n\n\\[Compare with previous versions?\\]\\(.*\\))?\
         $"
    )
    .unwrap();
}

fn ghostflow_user(user: types::FullUser) -> User {
    User {
        handle: user.username,
        name: user.name,
        email: user.email,
    }
}

fn ghostflow_pipeline_state(status: types::PipelineStatus) -> PipelineState {
    match status {
        types::PipelineStatus::Manual => PipelineState::Manual,
        types::PipelineStatus::Running
        | types::PipelineStatus::Pending
        | types::PipelineStatus::Created => PipelineState::InProgress,
        types::PipelineStatus::Canceled | types::PipelineStatus::Skipped => PipelineState::Canceled,
        types::PipelineStatus::Failed => PipelineState::Failed,
        types::PipelineStatus::Success => PipelineState::Success,
    }
}

fn ghostflow_pipeline(pipeline: types::Pipeline, repo: Repo, latest: Option<u64>) -> Pipeline {
    Pipeline {
        id: pipeline.id,
        state: ghostflow_pipeline_state(pipeline.status),
        commit: Commit {
            repo,
            id: CommitId::new(pipeline.sha),
            refname: pipeline.ref_,
            last_pipeline: latest,
        },
    }
}

fn ghostflow_job(job: types::PipelineJob, repo: Repo) -> PipelineJob {
    PipelineJob {
        id: job.id,
        state: ghostflow_pipeline_state(job.status),
        repo,
        stage: Some(job.stage),
        name: job.name,
    }
}

fn gitlab_state(state: CommitStatusState) -> api::projects::repository::commits::CommitStatusState {
    match state {
        CommitStatusState::Pending => {
            api::projects::repository::commits::CommitStatusState::Pending
        },
        CommitStatusState::Running => {
            api::projects::repository::commits::CommitStatusState::Running
        },
        CommitStatusState::Success => {
            api::projects::repository::commits::CommitStatusState::Success
        },
        CommitStatusState::Failed => api::projects::repository::commits::CommitStatusState::Failed,
    }
}

fn ghostflow_state(state: types::StatusState) -> CommitStatusState {
    match state {
        types::StatusState::Manual
        | types::StatusState::Skipped
        | types::StatusState::Created
        | types::StatusState::Canceled
        | types::StatusState::Pending
        | types::StatusState::Scheduled => CommitStatusState::Pending,
        types::StatusState::Running => CommitStatusState::Running,
        types::StatusState::Success => CommitStatusState::Success,
        types::StatusState::Failed => CommitStatusState::Failed,
    }
}

trait ReferenceTarget {
    fn sigil() -> char;
    fn id(&self) -> u64;
}

impl ReferenceTarget for types::Issue {
    fn sigil() -> char {
        '#'
    }

    fn id(&self) -> u64 {
        self.iid
    }
}

impl ReferenceTarget for types::MergeRequest {
    fn sigil() -> char {
        '!'
    }

    fn id(&self) -> u64 {
        self.iid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceLevel {
    Project,
    Namespace,
    Site,

}

impl ReferenceLevel {
    fn between(source: &types::Project, target: &types::Project) -> Self {
        if source.id == target.id {
            ReferenceLevel::Project
        } else if source.namespace == target.namespace {
            ReferenceLevel::Namespace
        } else {
            ReferenceLevel::Site
        }
    }

    fn to<T>(self, project: &types::Project, target: &T) -> String
    where
        T: ReferenceTarget,
    {
        match self {
            ReferenceLevel::Project => format!("{}{}", T::sigil(), target.id()),
            ReferenceLevel::Namespace => format!("{}{}{}", project.path, T::sigil(), target.id()),
            ReferenceLevel::Site => {
                format!(
                    "{}/{}{}{}",
                    project.namespace.path,
                    project.path,
                    T::sigil(),
                    target.id(),
                )
            },
        }
    }
}

impl Default for ReferenceLevel {
    fn default() -> Self {
        ReferenceLevel::Project
    }
}

pub struct GitlabService {
    gitlab: gitlab::Gitlab,
    user: User,
    domain: &'static str,
}

impl GitlabService {
    pub fn new(gitlab: gitlab::Gitlab) -> Result<Self, HostingServiceError> {
        let endpoint = api::users::CurrentUser::builder().build().unwrap();
        let gitlab_user: types::FullUser =
            endpoint.query(&gitlab).map_err(HostingServiceError::host)?;
        let user = ghostflow_user(gitlab_user);

        Ok(Self {
            user,
            gitlab,
            domain: "gitlab.invalid",
        })
    }

    pub fn gitlab(&self) -> &gitlab::Gitlab {
        &self.gitlab
    }

    fn query<Q, T>(&self, query: &Q) -> Result<T, HostingServiceError>
    where
        Q: api::Query<T, gitlab::Gitlab>,
        T: DeserializeOwned,
    {
        query.query(&self.gitlab).map_err(HostingServiceError::host)
    }

    fn full_project<'a, T>(&self, project: T) -> Result<types::Project, HostingServiceError>
    where
        T: Into<api::common::NameOrId<'a>>,
    {
        let endpoint = api::projects::Project::builder()
            .project(project)
            .build()
            .unwrap();
        self.query(&endpoint)
    }

    fn full_user_by_id(&self, id: u64) -> Result<types::User, HostingServiceError> {
        let endpoint = api::users::User::builder().user(id).build().unwrap();
        self.query(&endpoint)
    }

    fn full_user_by_name(&self, name: &str) -> Result<types::User, HostingServiceError> {
        let endpoint = api::users::Users::builder().username(name).build().unwrap();
        let users: Vec<types::User> = self.query(&endpoint)?;

        users
            .into_iter()
            .find(|user: &types::User| user.username == name)
            .ok_or_else(|| HostingServiceError::host(GitlabServiceError::no_such_user(name.into())))
    }

    fn user(&self, id: u64) -> Result<User, HostingServiceError> {
        self.full_user_by_id(id)
            .map(|user: types::User| ghostflow_user(user.for_domain(self.domain)))
    }

    fn user_by_name(&self, name: &str) -> Result<User, HostingServiceError> {
        self.full_user_by_name(name)
            .map(|user| ghostflow_user(user.for_domain(self.domain)))
    }

    fn repo_from_project(&self, project: types::Project) -> Result<Repo, HostingServiceError> {
        let parent_project = if let Some(ref upstream) = project.forked_from_project {
            let parent_project = self.full_project(upstream.id)?;
            Some(Box::new(self.repo_from_project(parent_project)?))
        } else {
            None
        };

        Ok(Repo {
            name: project.path_with_namespace,
            url: project.ssh_url_to_repo,
            forked_from: parent_project,
        })
    }

    fn commit_from_project(
        &self,
        project: types::Project,
        commit: &CommitId,
    ) -> Result<Commit, HostingServiceError> {
        let endpoint = api::projects::repository::commits::Commit::builder()
            .project(project.id)
            .commit(commit.as_str())
            .build()
            .unwrap();
        let commit: types::Commit = self.query(&endpoint)?;

        Ok(Commit {
            repo: self.repo_from_project(project)?,
            refname: None,
            id: CommitId::new(commit.id),
            last_pipeline: commit.last_pipeline.map(|pipe| pipe.id),
        })
    }

    fn gitlab_issue(
        &self,
        project: types::Project,
        issue: types::Issue,
        referrer: Option<&types::Project>,
    ) -> Result<Issue, HostingServiceError> {
        let reference = referrer.map_or(ReferenceLevel::Project, |source| {
            ReferenceLevel::between(source, &project)
        });

        Ok(Issue {
            reference: reference.to(&project, &issue),
            repo: self.repo_from_project(project)?,
            id: issue.iid,
            url: issue.web_url,
            labels: issue.labels,
        })
    }

    fn merge_request_from_project(
        &self,
        project: types::Project,
        id: u64,
    ) -> Result<MergeRequest, HostingServiceError> {
        let endpoint = api::projects::merge_requests::MergeRequest::builder()
            .project(project.id)
            .merge_request(id)
            .build()
            .unwrap();
        let mr: types::MergeRequest = endpoint
            .query(&self.gitlab)
            .map_err(HostingServiceError::host)?;
        let source_project = self.full_project(mr.source_project_id)?;
        let author = self.user(mr.author.id)?;

        let reference = ReferenceLevel::default().to(&project, &mr);

        let source_repo = self.repo_from_project(source_project.clone())?;
        let target_repo = self.repo_from_project(project.clone())?;
        let mr_commit = if let Some(ref sha) = &mr.sha {
            let commit_id = CommitId::new(sha);
            let source_commit = self.commit_from_project(source_project, &commit_id)?;
            let target_commit = self.commit_from_project(project, &commit_id)?;

            if target_commit.last_pipeline.is_some() {
                target_commit
            } else {
                let mut commit = source_commit;
                commit.refname = Some(mr.source_branch.clone());
                commit
            }
        } else {
            Commit {
                repo: source_repo.clone(),
                refname: Some(mr.source_branch.clone()),
                id: CommitId::new(""),
                last_pipeline: None,
            }
        };

        Ok(MergeRequest {
            source_repo: Some(source_repo),
            source_branch: mr.source_branch,
            target_repo,
            target_branch: mr.target_branch,
            id: mr.iid,
            url: mr.web_url,
            work_in_progress: mr.work_in_progress,
            description: mr.description.unwrap_or_default(),
            old_commit: None,
            commit: mr_commit,
            author,
            reference,
            remove_source_branch: mr.force_remove_source_branch.unwrap_or(false),
        })
    }

    fn sort_notes(&self, notes: Vec<types::Note>) -> Result<Vec<Comment>, HostingServiceError> {
        Ok(notes
            .into_iter()
            .map(|note| {
                Ok(Comment {
                    id: format!("{}", note.id),
                    is_system: note.system,
                    is_branch_update: note.system && MR_UPDATE_RE.is_match(&note.body),
                    created_at: note.created_at,
                    author: self.user(note.author.id)?,
                    content: note
                        .body
                        .replace('\u{200b}', ""),
                })
            })
            .collect::<Result<Vec<_>, HostingServiceError>>()?
            .into_iter()
            .sorted_by(|a, b| a.id.cmp(&b.id))
            .collect())
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
enum GitlabServiceError {
    #[error("failed to find a user named '{}'", name)]
    NoSuchUser { name: String },
}

impl GitlabServiceError {
    fn no_such_user(name: String) -> Self {
        GitlabServiceError::NoSuchUser {
            name,
        }
    }
}

impl From<GitlabServiceError> for HostingServiceError {
    fn from(gitlab: GitlabServiceError) -> Self {
        HostingServiceError::service(gitlab)
    }
}

impl HostingService for GitlabService {
    fn as_pipeline_service(self: Arc<Self>) -> Option<Arc<dyn HostedPipelineService>> {
        Some(self as Arc<dyn HostedPipelineService>)
    }

    fn service_user(&self) -> &User {
        &self.user
    }

    fn user(&self, project: &str, user: &str) -> Result<User, HostingServiceError> {
        let _ = self.full_project(project)?;
        self.user_by_name(user)
    }

    fn commit(&self, project: &str, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        let project = self.full_project(project)?;
        self.commit_from_project(project, commit)
    }

    fn merge_request(&self, project: &str, id: u64) -> Result<MergeRequest, HostingServiceError> {
        let project = self.full_project(project)?;
        self.merge_request_from_project(project, id)
    }

    fn repo(&self, project: &str) -> Result<Repo, HostingServiceError> {
        let project = self.full_project(project)?;
        self.repo_from_project(project)
    }

    fn get_mr_comments(&self, mr: &MergeRequest) -> Result<Vec<Comment>, HostingServiceError> {
        let endpoint = api::projects::merge_requests::notes::MergeRequestNotes::builder()
            .project(mr.target_repo.name.as_ref())
            .merge_request(mr.id)
            .build()
            .unwrap();
        let endpoint = api::paged(endpoint, api::Pagination::All);
        let notes: Vec<types::Note> = self.query(&endpoint)?;

        self.sort_notes(notes)
    }

    fn post_mr_comment(&self, mr: &MergeRequest, content: &str) -> Result<(), HostingServiceError> {
        let endpoint = api::projects::merge_requests::notes::CreateMergeRequestNote::builder()
            .project(mr.target_repo.name.as_ref())
            .merge_request(mr.id)
            .body(content)
            .build()
            .unwrap();
        let endpoint = api::ignore(endpoint);
        self.query(&endpoint)
    }

    fn get_commit_statuses(
        &self,
        commit: &Commit,
    ) -> Result<Vec<CommitStatus>, HostingServiceError> {
        let endpoint = api::projects::repository::commits::CommitStatuses::builder()
            .project(commit.repo.name.as_str())
            .commit(commit.id.as_str())
            .build()
            .unwrap();
        let endpoint = api::paged(endpoint, api::Pagination::All);
        let statuses = self.query(&endpoint)?;

        Ok(statuses
            .into_iter()
            .map(move |status: types::CommitStatus| {
                CommitStatus {
                    state: ghostflow_state(status.status),
                    author: ghostflow_user(status.author.for_domain(self.domain)),
                    refname: status.ref_,
                    name: status.name,
                    description: status.description.unwrap_or_default(),
                    target_url: status.target_url,
                }
            })
            .collect())
    }

    fn post_commit_status(&self, status: PendingCommitStatus) -> Result<(), HostingServiceError> {
        const REFS_HEADS_PREFIX: &str = "refs/heads/";
        const REFS_TAGS_PREFIX: &str = "refs/tags/";

        let refname = status.commit.refname.as_ref().map(|refname| {
            if let Some(head_name) = refname.strip_prefix(REFS_HEADS_PREFIX) {
                head_name
            } else if let Some(tag_name) = refname.strip_prefix(REFS_TAGS_PREFIX) {
                tag_name
            } else {
                refname
            }
        });

        let mut builder = api::projects::repository::commits::CreateCommitStatus::builder();
        builder
            .project(status.commit.repo.name.as_str())
            .commit(status.commit.id.as_str())
            .state(gitlab_state(status.state))
            .name(status.name)
            .description(status.description);

        if let Some(refname) = refname {
            builder.ref_(refname);
        }
        if let Some(target_url) = status.target_url {
            builder.target_url(target_url);
        }
        if let Some(last_pipeline) = status.commit.last_pipeline {
            builder.pipeline_id(last_pipeline);
        }

        let endpoint = builder.build().unwrap();
        let endpoint = api::ignore(endpoint);
        self.query(&endpoint)
    }

    fn get_mr_awards(&self, mr: &MergeRequest) -> Result<Vec<Award>, HostingServiceError> {
        let endpoint = api::projects::merge_requests::awards::MergeRequestAwards::builder()
            .project(mr.target_repo.name.as_ref())
            .merge_request(mr.id)
            .build()
            .unwrap();
        let endpoint = api::paged(endpoint, api::Pagination::All);

        self.query(&endpoint)?
            .into_iter()
            .map(|award: types::AwardEmoji| {
                let author = self.user(award.user.id)?;

                Ok(Award {
                    name: award.name,
                    author,
                })
            })
            .collect()
    }

    fn issues_closed_by_mr(&self, mr: &MergeRequest) -> Result<Vec<Issue>, HostingServiceError> {
        let target_name = &mr.target_repo.name;
        let target_project = self.full_project(target_name.as_str())?;

        let endpoint = api::projects::merge_requests::IssuesClosedBy::builder()
            .project(target_project.id)
            .merge_request(mr.id)
            .build()
            .unwrap();
        let endpoint = api::paged(endpoint, api::Pagination::All);
        let issues: Vec<types::Issue> = self.query(&endpoint)?;

        // Cache projects to reduce hitting the service so much.
        let projects = issues
            .iter()
            .map(|issue| issue.project_id)
            .unique()
            .map(|project_id| {
                self.full_project(project_id)
                    .map(|project| (project_id, project))
            })
            .collect::<Result<HashMap<_, _>, HostingServiceError>>();

        // Link to each issue.
        projects.and_then(|projects| {
            issues
                .into_iter()
                .map(|issue| {
                    let project = projects
                        .get(&issue.project_id)
                        .expect("the fetched project ID should exist");
                    self.gitlab_issue(project.clone(), issue, Some(&target_project))
                })
                .collect()
        })
    }

    fn add_issue_labels(&self, issue: &Issue, labels: &[&str]) -> Result<(), HostingServiceError> {
        let endpoint = {
            let mut endpoint = api::projects::issues::EditIssue::builder();
            endpoint.project(issue.repo.name.as_str()).issue(issue.id);

            for label in labels {
                endpoint.add_label(*label);
            }

            endpoint.build().unwrap()
        };
        let endpoint = api::ignore(endpoint);
        self.query(&endpoint)
    }
}

impl HostedPipelineService for GitlabService {
    fn pipelines_for_mr(
        &self,
        mr: &MergeRequest,
    ) -> Result<Option<Vec<Pipeline>>, HostingServiceError> {
        let source_pipelines = if let Some(source_repo) = mr.source_repo.as_ref() {
            let project = self.full_project(source_repo.name.as_str())?;

            if project.builds_access_level != types::AccessLevel::Disabled {
                let endpoint = api::projects::pipelines::Pipelines::builder()
                    .project(source_repo.name.as_str())
                    .ref_(mr.source_branch.as_str())
                    .sha(mr.commit.id.as_str())
                    .build()
                    .unwrap();
                let endpoint = api::paged(endpoint, api::Pagination::All);
                let pipelines: Vec<types::Pipeline> = self.query(&endpoint)?;
                let repo = self.repo_from_project(project)?;
                let latest = pipelines.iter().map(|pipeline| pipeline.id).max();
                pipelines
                    .into_iter()
                    .map(|pipeline| ghostflow_pipeline(pipeline, repo.clone(), latest))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let target_pipelines = {
            let project = self.full_project(mr.target_repo.name.as_str())?;

            if project.builds_access_level != types::AccessLevel::Disabled {
                let mr_ref = format!("refs/merge-requests/{}/head", mr.id);
                let endpoint = api::projects::pipelines::Pipelines::builder()
                    .project(mr.target_repo.name.as_str())
                    .ref_(mr_ref)
                    .sha(mr.commit.id.as_str())
                    .build()
                    .unwrap();
                let endpoint = api::paged(endpoint, api::Pagination::All);
                let pipelines: Vec<types::Pipeline> = self.query(&endpoint)?;
                let repo = self.repo_from_project(project)?;
                let latest = pipelines.iter().map(|pipeline| pipeline.id).max();
                pipelines
                    .into_iter()
                    .map(|pipeline| ghostflow_pipeline(pipeline, repo.clone(), latest))
                    .collect()
            } else {
                Vec::new()
            }
        };

        Ok(Some(
            source_pipelines
                .into_iter()
                .chain(target_pipelines.into_iter())
                .collect(),
        ))
    }

    fn pipeline_jobs(
        &self,
        pipeline: &Pipeline,
    ) -> Result<Option<Vec<PipelineJob>>, HostingServiceError> {
        let project = self.full_project(pipeline.commit.repo.name.as_str())?;

        if project.builds_access_level == types::AccessLevel::Disabled {
            return Ok(None);
        }

        let endpoint = api::projects::pipelines::PipelineJobs::builder()
            .project(pipeline.commit.repo.name.as_str())
            .pipeline(pipeline.id)
            .build()
            .unwrap();
        let endpoint = api::paged(endpoint, api::Pagination::All);
        let jobs: Vec<types::PipelineJob> = self.query(&endpoint)?;
        Ok(Some(
            jobs.into_iter()
                .map(|job| ghostflow_job(job, pipeline.commit.repo.clone()))
                .collect(),
        ))
    }

    fn trigger_job(
        &self,
        job: &PipelineJob,
        user: Option<&str>,
    ) -> Result<(), HostingServiceError> {
        if job.state.is_complete() {
            let endpoint = api::projects::jobs::RetryJob::builder()
                .project(job.repo.name.as_str())
                .job(job.id)
                .build()
                .unwrap();
            if let Some(user) = user {
                let endpoint = api::sudo(endpoint, user);
                let endpoint = api::ignore(endpoint);
                self.query(&endpoint)
            } else {
                let endpoint = api::ignore(endpoint);
                self.query(&endpoint)
            }
        } else {
            let endpoint = api::projects::jobs::PlayJob::builder()
                .project(job.repo.name.as_str())
                .job(job.id)
                .build()
                .unwrap();
            if let Some(user) = user {
                let endpoint = api::sudo(endpoint, user);
                let endpoint = api::ignore(endpoint);
                self.query(&endpoint)
            } else {
                let endpoint = api::ignore(endpoint);
                self.query(&endpoint)
            }
        }
    }
}

impl fmt::Debug for GitlabService {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GitlabService")
            .field("user", &self.user.handle)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::MR_UPDATE_RE;

    #[test]
    fn test_mr_update_re() {
        let comments = [
            "Added 1 commit:\n\n\
             * deadbeef0 - blah blah blah blah blah blah blab blah",
            "Added 4 commits:\n\n\
             * deadbeef1 - blah blab blah blah\n\
             * deadbeef2 - blah blab blah blah\n\
             * deadbeef3 - blah blab blah blah\n\
             * deadbeef4 - blah blah blah blah",
            "added 1 commit\n\n\
             * deadbeef5 - blah blah blah\
             \n\n[Compare with previous version](link_to_revision_diff)",
            "added 90 commits\n\n\
             * deadbeef6...deadbeef7 - 89 commits from branch `upstream:master`\n\
             * deadbeef8 - blah blah blah blah\
             \n\n[Compare with previous version](link_to_revision_diff)",
            "added 1 commit\n\n\
             <ul>\
             <li>deadbeef7 - blah blah blah blah</li>\
             </ul>\
             \n\n[Compare with previous version](link_to_revision_diff)",
            "added 18 commits\n\n\
             <ul>\
             <li>deadbeef8...deadbeef9 - 17 commits from branch <code>upstream:master</code></li>\
             <li>deadbeef10 - Merge remote-tracking branch &#39;origin/master&#39; into this_topic</li>\
             </ul>\
             \n\n[Compare with previous version](link_to_revision_diff)",
        ];

        for comment in comments.iter() {
            assert!(MR_UPDATE_RE.is_match(dbg!(comment)));
        }
    }
}
