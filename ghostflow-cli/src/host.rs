use std::sync::Arc;

use ghostflow::host::{Commit, HostingService, HostingServiceError, MergeRequest};
use git_workarea::{CommitId, GitContext, GitError};

pub const GHOSTFLOW_CONFIG_FILENAME: &str = ".ghostflow.yaml";

pub trait LocalService: HostingService + 'static {
    fn as_hosting_service(self: Arc<Self>) -> Arc<dyn HostingService>;

    fn git_context(&self) -> &GitContext;
    fn synth_merge_request(
        &self,
        head: &CommitId,
        target: &CommitId,
    ) -> Result<MergeRequest, HostingServiceError>;
    fn synth_commit(&self, commit: &CommitId) -> Result<Commit, HostingServiceError>;

    fn default_target(&self) -> Option<CommitId> {
        None
    }

    fn config(&self, commit: &CommitId) -> Result<Option<Vec<u8>>, GitError> {
        let cat_file = self
            .git_context()
            .git()
            .arg("cat-file")
            .arg("blob")
            .arg(format!("{}:{}", commit, GHOSTFLOW_CONFIG_FILENAME))
            .output()
            .map_err(|err| GitError::subcommand("cat-file blob", err))?;

        if cat_file.status.success() {
            Ok(Some(cat_file.stdout))
        } else {
            Ok(None)
        }
    }
}

mod ci;
pub use self::ci::{Ci, CiError};

mod local;
pub use self::local::{Local, LocalError};
