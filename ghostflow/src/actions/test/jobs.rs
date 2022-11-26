// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `test` action using JSON job files.
//!
//! This action drops a job file into a directory so that another tool can use it to perform the
//! testing.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use chrono::Utc;
use log::{error, info};
use rand::Rng;
use serde_json::Value;
use thiserror::Error;

use crate::host::{HostedProject, MergeRequest};

/// Errors which may occur when creating test jobs for a merge request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TestJobsError {
    /// Failure to create a job queue directory.
    #[error("failed to create the queue directory {}: {}", path.display(), source)]
    CreateDirectory {
        /// The path to the directory.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to create a test job file.
    #[error("failed to create a job file {}: {}", path.display(), source)]
    CreateJob {
        /// The path to the test job.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to write test job data.
    #[error("failed to write a job to {}: {}", path.display(), source)]
    WriteJob {
        /// The path to the test job.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: serde_json::Error,
    },
}

impl TestJobsError {
    fn create_directory(path: PathBuf, source: io::Error) -> Self {
        TestJobsError::CreateDirectory {
            path,
            source,
        }
    }

    fn create_job(path: PathBuf, source: io::Error) -> Self {
        TestJobsError::CreateJob {
            path,
            source,
        }
    }

    fn write_job(path: PathBuf, source: serde_json::Error) -> Self {
        TestJobsError::WriteJob {
            path,
            source,
        }
    }
}

type TestJobsResult<T> = Result<T, TestJobsError>;

/// Implementation of the `test` action.
#[derive(Debug)]
pub struct TestJobs {
    /// The directory to place test job files into.
    queue: PathBuf,
    /// The project to be tested.
    project: HostedProject,
    /// Whether the action should create informational comments or not.
    ///
    /// Errors always create comments.
    quiet: bool,
}

impl TestJobs {
    /// Create a new test action.
    pub fn new<Q>(queue: Q, project: HostedProject) -> TestJobsResult<Self>
    where
        Q: AsRef<Path>,
    {
        Self::new_impl(queue.as_ref(), project)
    }

    fn new_impl(queue: &Path, project: HostedProject) -> TestJobsResult<Self> {
        fs::create_dir_all(queue)
            .map_err(|err| TestJobsError::create_directory(queue.into(), err))?;

        Ok(Self {
            queue: queue.to_path_buf(),
            project,
            quiet: false,
        })
    }

    /// Reduce the number of comments made by the test action.
    pub fn quiet(&mut self) -> &mut Self {
        self.quiet = true;
        self
    }

    /// Test an update to a branch.
    pub fn test_update(&self, data: Value) -> TestJobsResult<()> {
        info!(
            target: "ghostflow/test/jobs",
            "queuing an update test job for {}",
            self.project.name,
        );

        self.queue_job(data)
    }

    /// Push a merge request for testing.
    pub fn test_mr(&self, mr: &MergeRequest, data: Value) -> TestJobsResult<()> {
        info!(
            target: "ghostflow/test/jobs",
            "queuing a test job for {}",
            mr.url,
        );

        self.queue_job(data)?;

        self.send_info_mr_comment(mr, "This topic has been queued for testing.");

        Ok(())
    }

    /// Queue a job into the target directory.
    fn queue_job(&self, data: Value) -> TestJobsResult<()> {
        let rndpart = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .map(char::from)
            .take(12)
            .collect::<String>();
        let job_path = self
            .queue
            .join(format!("{}-{}.json", Utc::now().to_rfc3339(), rndpart));
        let mut job_file = File::create(&job_path)
            .map_err(|err| TestJobsError::create_job(job_path.clone(), err))?;
        serde_json::to_writer(&mut job_file, &data)
            .map_err(|err| TestJobsError::write_job(job_path, err))?;

        Ok(())
    }

    /// Send a comment to a merge request.
    fn send_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if let Err(err) = self.project.service.post_mr_comment(mr, content) {
            error!(
                target: "ghostflow/test/jobs",
                "failed to post a comment to merge request: {}, {}: {:?}",
                self.project.name,
                mr.id,
                err,
            );
        }
    }

    /// Send an informational comment to a merge request.
    fn send_info_mr_comment(&self, mr: &MergeRequest, content: &str) {
        if !self.quiet {
            self.send_mr_comment(mr, content)
        }
    }
}

