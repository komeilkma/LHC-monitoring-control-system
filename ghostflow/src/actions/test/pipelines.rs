//! The `test` action using the hosting services CI APIs.
//!
//! This action uses the hosting service's CI services to manage testing.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
use regex::Regex;
use thiserror::Error;

use crate::host::{
    HostedPipelineService, HostingServiceError, MergeRequest, PipelineJob, PipelineState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobAction {
    Trigger,
    Ignore,
}

/// Actions which may be performed on pipelines for a merge request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestPipelinesAction {
    /// Start jobs awaiting manual intervention.
    ///
    /// This is the default.
    StartManual,
    /// Restart jobs which completed without success.
    RestartUnsuccessful,
    /// Restart jobs which completed with failure.
    RestartFailed,
    /// Restart all jobs.
    RestartAll,
}

impl Default for TestPipelinesAction {
    fn default() -> TestPipelinesAction {
        TestPipelinesAction::StartManual
    }
}

impl TestPipelinesAction {
    fn action_for(self, state: PipelineState) -> JobAction {
        // Always ignore in-progress jobs.
        if state == PipelineState::InProgress {
            return JobAction::Ignore;
        }

        match self {
            TestPipelinesAction::StartManual => {
                if state == PipelineState::Manual {
                    JobAction::Trigger
                } else {
                    JobAction::Ignore
                }
            },
            TestPipelinesAction::RestartUnsuccessful => {
                if state.is_complete() && state != PipelineState::Success {
                    JobAction::Trigger
                } else {
                    JobAction::Ignore
                }
            },
            TestPipelinesAction::RestartFailed => {
                if state == PipelineState::Failed {
                    JobAction::Trigger
                } else {
                    JobAction::Ignore
                }
            },
            TestPipelinesAction::RestartAll => {
                if state.is_complete() {
                    JobAction::Trigger
                } else {
                    JobAction::Ignore
                }
            },
        }
    }
}

/// Options for building a set of pipelines.
#[derive(Debug, Default, Clone, Builder)]
#[builder(setter(strip_option))]
pub struct TestPipelinesOptions<'a> {
    /// The action to perform on the jobs.
    ///
    /// By default, jobs awaiting manual intervention will be started.
    #[builder(default)]
    action: TestPipelinesAction,

    /// Only consider jobs from a certain stage.
    ///
    /// What a "stage" is depends on the hosting service. Some might support phased CI builds
    /// (e.g., GitLab) while others may just have different CI integrations from off-site (e.g.,
    /// GitHub).
    #[builder(setter(into), default)]
    stage: Option<Cow<'a, str>>,
    /// Only consider jobs matching one of a set regexes.
    #[builder(setter(name = "_jobs_matching"), private, default)]
    jobs_matching: Option<Vec<Regex>>,
    /// The user to start the jobs as.
    ///
    /// Note that services generally need elevated privileges in order to perform this action.
    /// There is no separate indication that this is the cause of the failure.
    #[builder(setter(into), default)]
    user: Option<Cow<'a, str>>,
}

impl<'a> TestPipelinesOptions<'a> {
    /// Create a builder for pipeline options.
    pub fn builder() -> TestPipelinesOptionsBuilder<'a> {
        TestPipelinesOptionsBuilder::default()
    }

    fn should_act_on(&self, job: &PipelineJob) -> bool {
        // The stage either needs to not matter or match.
        self.stage.as_ref().map_or(true, |stage| job.stage.as_ref().map_or(false, |job_stage| stage == job_stage))
            // The job name either needs to not matter or match any regex.
            && self.jobs_matching.as_ref().map_or(true, |regexes| {
                regexes.iter().any(|regex| regex.is_match(&job.name))
            })
    }
}

impl<'a> TestPipelinesOptionsBuilder<'a> {
    /// Only act on jobs matching a regular expression.
    ///
    /// Note that a job only needs to match a single regex to be acted upon.
    pub fn jobs_matching(&mut self, regex: Regex) -> &mut Self {
        self.jobs_matching
            .get_or_insert(None)
            .get_or_insert_with(Vec::new)
            .push(regex);
        self
    }

    /// Only act on jobs matching one of many regular expressions.
    ///
    /// Note that a job only needs to match a single regex to be acted upon.
    pub fn jobs_matching_any_of<I>(&mut self, iter: I) -> &mut Self
    where
        I: Iterator<Item = Regex>,
    {
        self.jobs_matching
            .get_or_insert(None)
            .get_or_insert_with(Vec::new)
            .extend(iter);
        self
    }
}

/// Errors which may occur when interacting with CI services for a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TestPipelinesError {
    /// The hosting service said there are no pipelines for the merge request.
    #[error("no pipelines are available for the merge request")]
    NoPipelinesAvailable,
    /// No pipelines were found for the merge request.
    ///
    /// There is a subtle difference between this and `NoAvailablePipelines`. This case is for when
    /// pipelines were valid, but none were found. The former is for no pipelines at all for the
    /// merge request (e.g., pipelines are disabled on the project for the merge request).
    #[error("no pipelines found for the merge request")]
    NoPipelines,
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
}

impl TestPipelinesError {
    fn no_pipelines_available() -> Self {
        TestPipelinesError::NoPipelinesAvailable
    }

    fn no_pipelines() -> Self {
        TestPipelinesError::NoPipelines
    }
}

type TestPipelinesResult<T> = Result<T, TestPipelinesError>;

/// Implementation of the `test` action using the service's CI support.
pub struct TestPipelines {
    /// The pipelines API for the service.
    service: Arc<dyn HostedPipelineService>,
}

impl fmt::Debug for TestPipelines {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TestPipelines").finish()
    }
}

impl TestPipelines {
    /// Create a new test action.
    pub fn new(service: Arc<dyn HostedPipelineService>) -> Self {
        Self {
            service,
        }
    }

    /// Push a merge request for testing.
    pub fn test_mr(
        &self,
        mr: &MergeRequest,
        options: &TestPipelinesOptions,
    ) -> TestPipelinesResult<()> {
        let pipelines = self
            .service
            .pipelines_for_mr(mr)?
            .ok_or_else(TestPipelinesError::no_pipelines_available)?;
        if pipelines.is_empty() {
            return Err(TestPipelinesError::no_pipelines());
        }
        let jobs = pipelines
            .iter()
            .map(|pipeline| {
                Ok(self
                    .service
                    .pipeline_jobs(pipeline)?
                    // A job-less pipeline is fine.
                    .unwrap_or_default())
            })
            .collect::<TestPipelinesResult<Vec<_>>>()?
            .into_iter()
            .flatten();
        let user = options.user.as_ref().map(AsRef::as_ref);

        // Act on the jobs in the pipeline.
        Ok(jobs
            // Filter out those the options don't want to consider.
            .filter(|job| options.should_act_on(job))
            .try_for_each(|job| {
                // Determine the action for the job.
                match options.action.action_for(job.state) {
                    JobAction::Ignore => Ok(()),
                    JobAction::Trigger => self.service.trigger_job(&job, user),
                }
            })?)
    }
}

#[cfg(test)]
mod tests {
    use crate::actions::test::pipelines::TestPipelinesAction;
    use crate::host::PipelineState;

    use super::JobAction;

    #[test]
    fn test_action_default() {
        assert_eq!(
            TestPipelinesAction::default(),
            TestPipelinesAction::StartManual,
        );
    }

    #[test]
    fn test_action_state_matrix() {
        let actions = [
            TestPipelinesAction::StartManual,
            TestPipelinesAction::RestartUnsuccessful,
            TestPipelinesAction::RestartFailed,
            TestPipelinesAction::RestartAll,
        ];
        let states = [
            PipelineState::Manual,
            PipelineState::InProgress,
            PipelineState::Canceled,
            PipelineState::Failed,
            PipelineState::Success,
        ];

        let i = JobAction::Ignore;
        let t = JobAction::Trigger;
        let expected = [
            // StartManual
            [t, i, i, i, i],
            // RestartUnsuccessful
            [i, i, t, t, i],
            // RestartFailed
            [i, i, i, t, i],
            // RestartAll
            [i, i, t, t, t],
        ];

        for (action_idx, action) in actions.iter().enumerate() {
            for (state_idx, &state) in states.iter().enumerate() {
                assert_eq!(
                    action.action_for(state),
                    expected[action_idx][state_idx],
                    "Unexpected result for {:?} on the {:?} state",
                    action,
                    state,
                );
            }
        }
    }
}
