use std::io::{self, Write};
use std::sync::Arc;

use ghostflow::host::*;
use git_workarea::{CommitId, GitContext};
use log::error;
use termcolor::{BufferWriter, Color, ColorSpec, WriteColor};
use thiserror::Error;

use crate::host::local::{git_color, Local};
use crate::host::LocalService;

#[derive(Debug, Error)]
#[non_exhaustive]
enum LocalGhostflowError {
    #[error("request for project '{}' is unsatisfiable", project)]
    UnsatisfiableProject { project: String },
    #[error("cannot perform {} on local repositories", action)]
    Unimplamentable { action: &'static str },
    #[error("failed to set terminal color: {}", source)]
    SetColor {
        #[source]
        source: io::Error,
    },
    #[error("failed to write to output buffer: {}", source)]
    Write {
        #[source]
        source: io::Error,
    },
    #[error("failed to write to the output: {}", source)]
    Output {
        #[source]
        source: io::Error,
    },
}

impl LocalGhostflowError {
    fn unsatisfiable_project(project: String) -> Self {
        LocalGhostflowError::UnsatisfiableProject {
            project,
        }
    }

    fn unimplementable(action: &'static str) -> Self {
        LocalGhostflowError::Unimplamentable {
            action,
        }
    }

    fn set_color(source: io::Error) -> Self {
        LocalGhostflowError::SetColor {
            source,
        }
    }

    fn write(source: io::Error) -> Self {
        LocalGhostflowError::Write {
            source,
        }
    }

    fn output(source: io::Error) -> Self {
        LocalGhostflowError::Output {
            source,
        }
    }
}

type LocalGhostflowResult<T> = Result<T, LocalGhostflowError>;

const STATUS_NAME_COLOR_CONFIG: &str = "color.ghostflow.status";
const STATUS_NAME_COLOR_DEFAULT: Color = Color::Cyan;
const STATUS_SUCCESS_COLOR_CONFIG: &str = "color.ghostflow.success";
const STATUS_SUCCESS_COLOR_DEFAULT: Color = Color::Green;
const STATUS_FAILED_COLOR_CONFIG: &str = "color.ghostflow.failed";
const STATUS_FAILED_COLOR_DEFAULT: Color = Color::Red;

impl Local {
    fn check_project(&self, project: &str) -> LocalGhostflowResult<()> {
        if project == self.repo.name {
            Ok(())
        } else {
            Err(LocalGhostflowError::unsatisfiable_project(project.into()))
        }
    }

    fn git_color(&self, config: &str) -> Option<ColorSpec> {
        let color = match self.git_config(config) {
            Ok(Some(value)) => value,
            Ok(None) => return None,
            Err(err) => {
                error!("{:?}", err);
                return None;
            },
        };

        Some(git_color::parse(&color))
    }
}

impl From<LocalGhostflowError> for HostingServiceError {
    fn from(err: LocalGhostflowError) -> Self {
        HostingServiceError::host(err)
    }
}

/// A hosting service.
impl HostingService for Local {
    /// The user the service is acting as.
    fn service_user(&self) -> &User {
        &self.user
    }

    fn user(&self, project: &str, user: &str) -> Result<User, HostingServiceError> {
        self.check_project(project)?;
        if user == self.user.name {
            Ok(self.user.clone())
        } else {
            Err(LocalGhostflowError::unimplementable("query non-owner user").into())
        }
    }

    fn commit(&self, project: &str, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        self.check_project(project)?;

        Ok(Commit {
            repo: self.repo.clone(),
            refname: None,
            id: commit.clone(),
            last_pipeline: None,
        })
    }

    fn merge_request(&self, project: &str, _: u64) -> Result<MergeRequest, HostingServiceError> {
        self.check_project(project)?;
        Err(LocalGhostflowError::unimplementable("get merge request").into())
    }

    fn repo(&self, project: &str) -> Result<Repo, HostingServiceError> {
        self.check_project(project)?;
        Ok(self.repo.clone())
    }

    fn get_mr_comments(&self, mr: &MergeRequest) -> Result<Vec<Comment>, HostingServiceError> {
        self.check_project(&mr.target_repo.name)?;
        Ok(Vec::new())
    }

    fn post_mr_comment(&self, mr: &MergeRequest, content: &str) -> Result<(), HostingServiceError> {
        self.check_project(&mr.target_repo.name)?;
        println!("{}", content);
        Ok(())
    }

    fn get_commit_statuses(
        &self,
        commit: &Commit,
    ) -> Result<Vec<CommitStatus>, HostingServiceError> {
        self.check_project(&commit.repo.name)?;
        Ok(Vec::new())
    }

    fn post_commit_status(&self, status: PendingCommitStatus) -> Result<(), HostingServiceError> {
        self.check_project(&status.commit.repo.name)?;

        let (state_color, state_default, state_str) = match status.state {
            // We don't want to report these states to stdout.
            CommitStatusState::Pending | CommitStatusState::Running => return Ok(()),
            CommitStatusState::Success => {
                (
                    STATUS_SUCCESS_COLOR_CONFIG,
                    STATUS_SUCCESS_COLOR_DEFAULT,
                    "success",
                )
            },
            CommitStatusState::Failed => {
                (
                    STATUS_FAILED_COLOR_CONFIG,
                    STATUS_FAILED_COLOR_DEFAULT,
                    "failed",
                )
            },
        };

        let writer = BufferWriter::stdout(self.color);
        let mut buf = writer.buffer();

        if buf.supports_color() {
            if let Some(color) = self.git_color("color.diff.commit") {
                buf.set_color(&color)
                    .map_err(LocalGhostflowError::set_color)?;
            }
        }
        write!(&mut buf, "commit {}", status.commit.id).map_err(LocalGhostflowError::write)?;

        if let Some(refname) = status.commit.refname.as_ref() {
            if buf.supports_color() {
                if let Some(color) = self.git_color("color.decorate.branch") {
                    buf.set_color(&color)
                        .map_err(LocalGhostflowError::set_color)?;
                }
            }
            write!(&mut buf, " ({})", refname).map_err(LocalGhostflowError::write)?;
        }
        writeln!(&mut buf).map_err(LocalGhostflowError::write)?;

        if buf.supports_color() {
            let color = self.git_color(STATUS_NAME_COLOR_CONFIG).unwrap_or_else(|| {
                let mut spec = ColorSpec::new();
                spec.set_fg(Some(STATUS_NAME_COLOR_DEFAULT));
                spec
            });
            buf.set_color(&color)
                .map_err(LocalGhostflowError::set_color)?;
        }
        writeln!(&mut buf, "status {}", state_str).map_err(LocalGhostflowError::write)?;

        if buf.supports_color() {
            let color = self.git_color(state_color).unwrap_or_else(|| {
                let mut spec = ColorSpec::new();
                spec.set_fg(Some(state_default));
                spec
            });
            buf.set_color(&color)
                .map_err(LocalGhostflowError::set_color)?;
        }
        writeln!(&mut buf, "state {}", state_str).map_err(LocalGhostflowError::write)?;

        if buf.supports_color() {
            buf.set_color(&ColorSpec::new())
                .map_err(LocalGhostflowError::set_color)?;
        }
        writeln!(
            &mut buf,
            "\n    {}",
            status.description.replace('\n', "\n    "),
        )
        .map_err(LocalGhostflowError::write)?;

        writer.print(&buf).map_err(LocalGhostflowError::output)?;

        Ok(())
    }

    fn get_mr_awards(&self, mr: &MergeRequest) -> Result<Vec<Award>, HostingServiceError> {
        self.check_project(&mr.target_repo.name)?;
        Ok(Vec::new())
    }

    fn issues_closed_by_mr(&self, mr: &MergeRequest) -> Result<Vec<Issue>, HostingServiceError> {
        self.check_project(&mr.target_repo.name)?;
        Ok(Vec::new())
    }

    fn add_issue_labels(&self, issue: &Issue, _: &[&str]) -> Result<(), HostingServiceError> {
        self.check_project(&issue.repo.name)?;
        Err(LocalGhostflowError::unimplementable("add issue labels").into())
    }
}

impl LocalService for Local {
    fn as_hosting_service(self: Arc<Self>) -> Arc<dyn HostingService> {
        self
    }

    fn git_context(&self) -> &GitContext {
        &self.ctx
    }

    fn synth_commit(&self, commit: &CommitId) -> Result<Commit, HostingServiceError> {
        self.commit(&self.repo.name, commit)
    }

    fn synth_merge_request(
        &self,
        head: &CommitId,
        target: &CommitId,
    ) -> Result<MergeRequest, HostingServiceError> {
        Ok(MergeRequest {
            source_repo: Some(self.repo.clone()),
            source_branch: head.as_str().into(),
            target_repo: self.repo.clone(),
            target_branch: target.as_str().into(),
            id: 0,
            url: "local".into(),
            work_in_progress: false,
            description: String::new(),
            old_commit: None,
            commit: Commit {
                repo: self.repo.clone(),
                refname: Some(head.as_str().into()),
                id: head.clone(),
                last_pipeline: None,
            },
            author: self.user.clone(),
            reference: String::new(),
            remove_source_branch: false,
        })
    }
}
