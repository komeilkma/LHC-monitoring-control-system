use crate::host::types::{Commit, MergeRequest, Repo};
use crate::host::{HostingService, HostingServiceError};

/// States for a pipeline as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    /// The pipeline is awaiting manual intervention.
    Manual,
    /// The pipeline is in progress.
    InProgress,
    /// The pipeline has been canceled.
    Canceled,
    /// The pipeline has failed.
    Failed,
    /// The pipeline has succeeded.
    Success,
}

impl PipelineState {
    /// Whether the state represents completion or not.
    pub fn is_complete(self) -> bool {
        match self {
            PipelineState::Manual | PipelineState::InProgress => false,
            PipelineState::Canceled | PipelineState::Success | PipelineState::Failed => true,
        }
    }
}

/// A pipeline for testing or other actions for a repository.
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// The current state of the pipeline.
    pub state: PipelineState,
    /// The commit the pipeline is for.
    pub commit: Commit,
    /// The ID of the pipeline.
    pub id: u64,
}

/// A job within a pipeline.
#[derive(Debug, Clone)]
pub struct PipelineJob {
    /// The repository the job belongs to.
    pub repo: Repo,
    /// The state of the job.
    pub state: PipelineState,
    /// The stage of the job.
    pub stage: Option<String>,
    /// The name of the job.
    pub name: String,
    /// The ID of the job.
    pub id: u64,
}

/// Pipeline information for a hosting service.
///
/// Not all services have this, so it is a separate trait.
pub trait HostedPipelineService: HostingService {
    /// Get all of the pipelines for a merge request.
    fn pipelines_for_mr(
        &self,
        mr: &MergeRequest,
    ) -> Result<Option<Vec<Pipeline>>, HostingServiceError>;
    /// Get all of the jobs for a pipeline.
    fn pipeline_jobs(
        &self,
        pipeline: &Pipeline,
    ) -> Result<Option<Vec<PipelineJob>>, HostingServiceError>;
    /// Trigger a job.
    ///
    /// Start running a job on the CI service. Note that jobs currently running have a
    /// service-defined behavior (i.e., trigger requests may either be ignored or the job canceled
    /// and restarted).
    ///
    /// Note that if the service does not support starting as a specific user, the `user` argument
    /// may be silently ignored.
    fn trigger_job(&self, job: &PipelineJob, user: Option<&str>)
        -> Result<(), HostingServiceError>;
}
