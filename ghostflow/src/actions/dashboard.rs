

//! The `dashboard` action.
//!
//! This action adds a status to a merge request or commit with a link to the dashboard showing CI
//! results for the code.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use thiserror::Error;

use crate::host::{Commit, CommitStatusState, HostingService, HostingServiceError, MergeRequest};
use crate::utils::TemplateString;

/// Errors which may occur when checking a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DashboardError {
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
}

type DashboardActionResult<T> = Result<T, DashboardError>;

/// A `dashboard` action.
///
/// Some projects use an external "dashboard" for collating results from testing. This action posts
/// a commit status to a merge request or commit containing a link to the external dashboard. The
/// status is always in a passing state; no attempt is made to try and synchronize the status'
/// state with that of the dashboard itself.
///
/// Each of the `status_name`, `url`, and `description` fields are "templates" which may use
/// `{field}` references to expand to values within the context of the commit or merge request (see
/// the relevant methods for the available expansions. Field names which are unknown are ignored
/// and expand to nothing.
///
/// No escaping mechanisms are currently available.
pub struct Dashboard {
    /// The service which hosts the project.
    service: Arc<dyn HostingService>,
    /// The name of the status to use.
    status_name: TemplateString,
    /// The URL to use use to post to the dashboard.
    url: TemplateString,
    /// The description to use for the status.
    description: TemplateString,
}

impl Dashboard {
    /// Create a new dashboard action.
    pub fn new<S, U, D>(
        service: Arc<dyn HostingService>,
        status_name: S,
        url: U,
        description: D,
    ) -> Self
    where
        S: Into<String>,
        U: Into<String>,
        D: Into<String>,
    {
        Dashboard {
            service,
            status_name: TemplateString::new(status_name.into()),
            url: TemplateString::new(url.into()),
            description: TemplateString::new(description.into()),
        }
    }

    fn status_name(&self, data: &HashMap<&str, Cow<str>>) -> String {
        self.status_name.replace(data)
    }

    fn url(&self, data: &HashMap<&str, Cow<str>>) -> String {
        self.url.replace(data)
    }

    fn description(&self, data: &HashMap<&str, Cow<str>>) -> String {
        self.description.replace(data)
    }

    /// Post a dashboard status for a commit.
    ///
    /// Available replacements:
    ///
    ///   - `commit`
    ///   - `refname` (if available)
    ///   - `branch_name` (if `refname` starts with `refs/heads/`)
    ///   - `tag_name` (if `refname` starts with `refs/tags/`)
    ///   - `pipeline_id` (if available)
    pub fn post_for_commit(&self, commit: &Commit) -> DashboardActionResult<()> {
        let data = {
            let mut data: HashMap<_, _> = [("commit", Cow::Borrowed(commit.id.as_str()))]
                .iter()
                .cloned()
                .collect();
            if let Some(ref refname) = commit.refname {
                let refs_heads_prefix = "refs/heads/";
                if let Some(head_name) = refname.strip_prefix(refs_heads_prefix) {
                    data.insert("branch_name", Cow::Borrowed(head_name));
                }
                let refs_tags_prefix = "refs/tags/";
                if let Some(tag_name) = refname.strip_prefix(refs_tags_prefix) {
                    data.insert("tag_name", Cow::Borrowed(tag_name));
                }
                data.insert("refname", Cow::Borrowed(refname));
            }
            if let Some(pipeline_id) = commit.last_pipeline {
                data.insert("pipeline_id", format!("{}", pipeline_id).into());
            }
            data
        };

        let status_name = self.status_name(&data);
        let description = self.description(&data);
        let url = self.url(&data);

        let mut status =
            commit.create_commit_status(CommitStatusState::Success, &status_name, &description);
        status.target_url = Some(&url);

        Ok(self.service.post_commit_status(status)?)
    }

    /// Post a dashboard status for a merge request.
    ///
    /// Available replacements:
    ///
    ///   - `source_branch`
    ///   - `target_branch`
    ///   - `commit`
    ///   - `mr_id`
    ///   - `pipeline_id` (if available)
    pub fn post_for_mr(&self, mr: &MergeRequest) -> DashboardActionResult<()> {
        let id_str = format!("{}", mr.id);
        let data = {
            let mut data: HashMap<_, _> = [
                ("source_branch", Cow::Borrowed(mr.source_branch.as_ref())),
                ("target_branch", Cow::Borrowed(mr.target_branch.as_ref())),
                ("commit", Cow::Borrowed(mr.commit.id.as_str())),
                ("mr_id", Cow::Borrowed(id_str.as_ref())),
            ]
            .iter()
            .cloned()
            .collect();
            if let Some(pipeline_id) = mr.commit.last_pipeline {
                data.insert("pipeline_id", format!("{}", pipeline_id).into());
            }
            data
        };

        let status_name = self.status_name(&data);
        let description = self.description(&data);
        let url = self.url(&data);

        let mut status =
            mr.create_commit_status(CommitStatusState::Success, &status_name, &description);
        status.target_url = Some(&url);

        Ok(self.service.post_commit_status(status)?)
    }

    /// Post a dashboard status for a merge request with altered data.
    ///
    /// This can be used to post a status to the main MR, but using another commit to provide
    /// different replacement information (e.g., posting a dashboard status for a related commit to
    /// the location the MR itself expects).
    ///
    /// Available replacements:
    ///
    ///   - `source_branch`
    ///   - `target_branch`
    ///   - `commit`
    ///   - `mr_id`
    ///   - `pipeline_id` (if available)
    pub fn post_for_mr_altered(
        &self,
        mr: &MergeRequest,
        commit: &Commit,
    ) -> DashboardActionResult<()> {
        let id_str = format!("{}", mr.id);
        let data = {
            let mut data: HashMap<_, _> = [
                ("source_branch", Cow::Borrowed(mr.source_branch.as_ref())),
                ("target_branch", Cow::Borrowed(mr.target_branch.as_ref())),
                ("commit", Cow::Borrowed(commit.id.as_str())),
                ("mr_id", Cow::Borrowed(id_str.as_ref())),
            ]
            .iter()
            .cloned()
            .collect();
            if let Some(pipeline_id) = commit.last_pipeline {
                data.insert("pipeline_id", format!("{}", pipeline_id).into());
            }
            data
        };

        let status_name = self.status_name(&data);
        let description = self.description(&data);
        let url = self.url(&data);

        let mut status =
            mr.create_commit_status(CommitStatusState::Success, &status_name, &description);
        status.target_url = Some(&url);

        Ok(self.service.post_commit_status(status)?)
    }
}

impl Debug for Dashboard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Dashboard")
            .field("status_name", &self.status_name)
            .field("url", &self.url)
            .field("description", &self.description)
            .finish()
    }
}
