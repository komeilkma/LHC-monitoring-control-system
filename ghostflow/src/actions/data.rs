

//! The `data` action.
//!
//! This action fetches data objects pushed to a repository under `refs/data/` (or another
//! namespace) and pushes them using `rsync` to multiple destinations.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;

use digest::Digest;
use git_workarea::{GitContext, GitError};
use itertools::Itertools;
use log::{error, info, warn};
use md5::Md5;
use sha2::{Sha256, Sha512};
use tempfile::TempDir;
use thiserror::Error;

use crate::host::Repo;

/// Errors which may occur when handling data refs.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DataError {
    /// Failure to list data refs in a remote.
    #[error("failed to list data refs {} in {}: {}", remote, remote, output)]
    ListDataRefs {
        /// The name of the remote.
        remote: String,
        /// The glob used to list refs.
        data_glob: String,
        /// Output from `git ls-remote`.
        output: String,
    },
    /// Failure to delete a remote data ref.
    #[error("failed to delete data refs \"{}\" from {}: {}", refnames.iter().format("\", \""), url, output)]
    DeleteRemoteRef {
        /// The refnames to delete.
        refnames: Vec<String>,
        /// The URL of the remote.
        url: String,
        /// Output from `git push`.
        output: String,
    },
    /// Failure to create a temporary directory.
    #[error("failed to create temporary directory under {}: {}", path.display(), source)]
    CreateTempDirectory {
        /// The parent directory for the requested temporary directory.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to create a directory.
    #[error("failed to create directory {}: {}", path.display(), source)]
    CreateDirectory {
        /// The path to the directory.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to create a file.
    #[error("failed to create data file {}: {}", path.display(), source)]
    CreateFile {
        /// The path to the file.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to write the data object to a file.
    #[error("failed to write data file {}: {}", path.display(), source)]
    WriteFile {
        /// The path to the file.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to construct a sync command.
    #[error("failed to construct sync command for {}: {}", command, source)]
    SyncCommand {
        /// The name of the command.
        command: &'static str,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to perform an `rsync` operation.
    #[error(
        "failed to rsync data to {} (status: {:?}): {}",
        destination,
        status,
        output
    )]
    Rsync {
        /// The requested destination.
        destination: String,
        /// The return code from `rsync`.
        status: Option<i32>,
        /// Output from `rsync`.
        output: String,
    },
    /// Failure to detect the object type of a ref.
    #[error("failed to get the type of {}: {}", refname, output)]
    ObjectType {
        /// The refname requested.
        refname: String,
        /// Output from `git cat-file`.
        output: String,
    },
    /// Failure to get the contents of a ref.
    #[error("failed to get the contents of {}: {}", refname, output)]
    ObjectContents {
        /// The refname requested.
        refname: String,
        /// Output from `git cat-file`.
        output: String,
    },
    /// A data ref points to an unsupported object type.
    #[error("unsupported data object type for {}: {}", refname, type_)]
    UnsupportedObjectType {
        /// The refname checked.
        refname: String,
        /// The type of the ref.
        type_: String,
    },
    /// Failure to delete a data ref.
    #[error("failed to delete data ref {}: {}", refname, output)]
    DeleteDataRef {
        /// The refname to delete.
        refname: String,
        /// Output from `git update-ref`.
        output: String,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
}

impl DataError {
    fn list_data_refs(remote: String, data_glob: String, output: &[u8]) -> Self {
        DataError::ListDataRefs {
            remote,
            data_glob,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn delete_remote_ref(refnames: Vec<&str>, url: String, output: &[u8]) -> Self {
        DataError::DeleteRemoteRef {
            refnames: refnames.into_iter().map(Into::into).collect(),
            url,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn create_temp_directory(path: PathBuf, source: io::Error) -> Self {
        DataError::CreateTempDirectory {
            path,
            source,
        }
    }

    fn create_directory(path: PathBuf, source: io::Error) -> Self {
        DataError::CreateDirectory {
            path,
            source,
        }
    }

    fn create_file(path: PathBuf, source: io::Error) -> Self {
        DataError::CreateFile {
            path,
            source,
        }
    }

    fn write_file(path: PathBuf, source: io::Error) -> Self {
        DataError::WriteFile {
            path,
            source,
        }
    }

    fn sync_command(command: &'static str, source: io::Error) -> Self {
        DataError::SyncCommand {
            command,
            source,
        }
    }

    fn rsync(destination: String, status: Option<i32>, output: &[u8]) -> Self {
        DataError::Rsync {
            destination,
            status,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn object_type(refname: String, output: &[u8]) -> Self {
        DataError::ObjectType {
            refname,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn object_contents(refname: String, output: &[u8]) -> Self {
        DataError::ObjectContents {
            refname,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn unsupported_object_type(refname: String, type_: String) -> Self {
        DataError::UnsupportedObjectType {
            refname,
            type_,
        }
    }

    fn delete_data_ref(refname: String, output: &[u8]) -> Self {
        DataError::DeleteDataRef {
            refname,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type DataResult<T> = Result<T, DataError>;

/// Implementation of the `data` action.
#[derive(Debug)]
pub struct Data {
    /// The context to use for fetching data refs.
    ctx: GitContext,
    /// The `rsync` destinations to upload data to.
    destinations: Vec<String>,
    /// The reference namespace to use for data.
    ref_namespace: String,
    /// Whether to keep refs on remotes or not.
    keep_refs: bool,
}

/// The result of the data action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataActionResult {
    /// No data was found in the repository.
    NoData,
    /// No destinations are configured.
    NoDestinations,
    /// Data was successfully pushed to the destinations.
    DataPushed,
}

impl Data {
    /// Create a new data action.
    pub fn new(ctx: GitContext) -> Self {
        Self {
            ctx,
            destinations: Vec::new(),
            ref_namespace: "data".into(),
            keep_refs: false,
        }
    }

    /// Add a destination for the data.
    pub fn add_destination<D>(&mut self, destination: D) -> &mut Self
    where
        D: Into<String>,
    {
        self.destinations.push(destination.into());
        self
    }

    /// Preserve data refs found on remote servers.
    ///
    /// By default, data which has been successfully fetched will be deleted from the given remote.
    pub fn keep_refs(&mut self) -> &mut Self {
        self.keep_refs = true;
        self
    }

    /// Use the given ref namespace for data objects.
    ///
    /// By default, `data` is used to look under `refs/data/`.
    pub fn ref_namespace<R>(&mut self, ref_namespace: R) -> &mut Self
    where
        R: Into<String>,
    {
        self.ref_namespace = ref_namespace.into();
        self
    }

    /// Fetch all data from a repository and mirror it to the destinations.
    pub fn fetch_data(&self, repo: &Repo) -> DataResult<DataActionResult> {
        info!(
            target: "ghostflow/data",
            "checking for data in {}",
            repo.url,
        );

        let data_ref_ns = format!("refs/{}/", self.ref_namespace);
        let data_ref_glob = format!("{}*", data_ref_ns);

        // Find all of the data refs in the repository.
        let ls_remote = self
            .ctx
            .git()
            .arg("ls-remote")
            .arg("--quiet")
            .arg("--exit-code")
            .arg(&repo.url)
            .arg(&data_ref_glob)
            .output()
            .map_err(|err| GitError::subcommand("ls-remote", err))?;
        if !ls_remote.status.success() {
            if let Some(2) = ls_remote.status.code() {
                return Ok(DataActionResult::NoData);
            } else {
                return Err(DataError::list_data_refs(
                    repo.url.clone(),
                    data_ref_glob,
                    &ls_remote.stderr,
                ));
            }
        }
        let remote_data_refs = String::from_utf8_lossy(&ls_remote.stdout);
        // Get the names of all the refs.
        #[allow(clippy::manual_split_once)]
        let data_refs = remote_data_refs
            .lines()
            // Extract just the refname from the output.
            // XXX(rust-1.52): use `line.split_once('\t').map(|t| t.1)`
            .filter_map(|line| line.splitn(2, '\t').nth(1))
            .collect::<Vec<_>>();

        info!(
            target: "ghostflow/data",
            "fetching data from {}",
            repo.url,
        );

        // Fetch the data from the remote repository.
        self.ctx
            .force_fetch_into(&repo.url, &data_ref_glob, &data_ref_glob)?;

        // If we have nowhere to put the data, leave it on the server and locally.
        if self.destinations.is_empty() {
            return Ok(DataActionResult::NoDestinations);
        }

        if !self.keep_refs {
            // Delete the refs from the remote server.
            let delete_refs = self
                .ctx
                .git()
                .arg("push")
                .arg("--atomic")
                .arg("--porcelain")
                .arg("--delete")
                .arg(&repo.url)
                .args(&data_refs)
                .output()
                .map_err(|err| GitError::subcommand("push", err))?;
            if !delete_refs.status.success() {
                return Err(DataError::delete_remote_ref(
                    data_refs,
                    repo.url.clone(),
                    &delete_refs.stderr,
                ));
            }
        }

        // Create a temporary directory to store data objects to push to the destinations.
        let tempdir = TempDir::new_in(self.ctx.gitdir())
            .map_err(|err| DataError::create_temp_directory(self.ctx.gitdir().into(), err))?;

        // Compute the number of path parts in the ref namespace.
        let namespace_parts = 1 + self.ref_namespace.chars().filter(|&ch| ch == '/').count();
        let mut valid_refs = Vec::new();
        for data_ref in data_refs {
            // Extract the algorithm and hash parts from the refname.
            let ref_parts = data_ref
                .splitn(3 + namespace_parts, '/')
                // Skip the `refs/.../` bit.
                .skip(1 + namespace_parts)
                .tuples()
                .next();
            let (digest_str, expected_hash) = if let Some(bits) = ref_parts {
                bits
            } else {
                warn!(
                    target: "ghostflow/data",
                    "unsupported refname {}",
                    data_ref,
                );

                self.delete_ref(data_ref)?;
                continue;
            };
            let (contents, hash) = match digest_str {
                "MD5" => self.hash_blob::<Md5>(data_ref)?,
                "SHA256" => self.hash_blob::<Sha256>(data_ref)?,
                "SHA512" => self.hash_blob::<Sha512>(data_ref)?,
                _ => {
                    error!(
                        target: "ghostflow/data",
                        "unsupported digest algorithm {}; ignoring",
                        digest_str,
                    );
                    continue;
                },
            };
            let hash_matches = expected_hash == hash;

            if hash_matches {
                let output_dir = tempdir.path().join(digest_str);
                fs::create_dir_all(&output_dir)
                    .map_err(|err| DataError::create_directory(output_dir.clone(), err))?;

                let output_path = output_dir.join(&hash);

                // Write to the data file.
                {
                    let mut output_file = OpenOptions::new()
                        .mode(0o444)
                        .write(true)
                        .create_new(true)
                        .open(&output_path)
                        .map_err(|err| DataError::create_file(output_path.clone(), err))?;

                    output_file
                        .write_all(&contents)
                        .map_err(|err| DataError::write_file(output_path.clone(), err))?;
                }

                valid_refs.push(data_ref);
            } else {
                warn!(
                    target: "ghostflow/data",
                    "failed to verify {} hash; expected {}, actually {}",
                    data_ref,
                    expected_hash,
                    hash,
                );

                self.lenient_delete_ref(data_ref);
            }
        }

        let mut source = tempdir.path().as_os_str().to_os_string();
        // We want to sync the contents of this directory, so add the trailing slash.
        source.push("/");
        self.destinations
            .iter()
            .map(|destination| {
                // Push the data to the remote server.
                let rsync = Command::new("rsync")
                    .arg("--recursive")
                    .arg("--perms")
                    .arg("--times")
                    .arg("--verbose")
                    .arg(&source)
                    .arg(destination)
                    .output()
                    .map_err(|err| DataError::sync_command("rsync", err))?;
                if !rsync.status.success() {
                    return Err(DataError::rsync(
                        destination.into(),
                        rsync.status.code(),
                        &rsync.stderr,
                    ));
                }

                Ok(())
            })
            .collect::<Vec<DataResult<_>>>()
            .into_iter()
            .collect::<DataResult<Vec<_>>>()?;

        if !self.keep_refs {
            valid_refs
                .into_iter()
                .for_each(|refname| self.lenient_delete_ref(refname));
        }

        Ok(DataActionResult::DataPushed)
    }

    /// Hash a git object using a digest.
    fn hash_blob<D>(&self, refname: &str) -> DataResult<(Vec<u8>, String)>
    where
        D: Digest,
        digest::Output<D>: std::fmt::LowerHex,
    {
        let contents = self.blob_contents(refname)?;

        // Compute the hash of the contents.
        let mut digest = D::new();
        digest.update(&contents);
        Ok((contents, format!("{:x}", digest.finalize())))
    }

    fn blob_contents(&self, refname: &str) -> DataResult<Vec<u8>> {
        // Get the type of the object.
        let cat_file = self
            .ctx
            .git()
            .arg("cat-file")
            .arg("-t")
            .arg(refname)
            .output()
            .map_err(|err| GitError::subcommand("cat-file -t", err))?;
        if !cat_file.status.success() {
            return Err(DataError::object_type(refname.into(), &cat_file.stderr));
        }
        let object_type = String::from_utf8_lossy(&cat_file.stdout);

        if object_type.trim() == "blob" {
            // Get the contents of the file.
            let cat_file = self
                .ctx
                .git()
                .arg("cat-file")
                .arg("blob")
                .arg(refname)
                .output()
                .map_err(|err| GitError::subcommand("cat-file blob", err))?;
            if !cat_file.status.success() {
                return Err(DataError::object_contents(refname.into(), &cat_file.stderr));
            }

            Ok(cat_file.stdout)
        } else {
            // Other object types are not supported.
            Err(DataError::unsupported_object_type(
                refname.into(),
                object_type.trim().into(),
            ))
        }
    }

    /// Delete a local ref.
    fn delete_ref(&self, refname: &str) -> DataResult<()> {
        let update_ref = self
            .ctx
            .git()
            .arg("update-ref")
            .arg("-d")
            .arg(refname)
            .output()
            .map_err(|err| GitError::subcommand("update-ref -d", err))?;
        if !update_ref.status.success() {
            return Err(DataError::delete_data_ref(
                refname.into(),
                &update_ref.stderr,
            ));
        }

        Ok(())
    }

    /// Delete a local ref, ignoring errors.
    fn lenient_delete_ref(&self, refname: &str) {
        let _ = self.delete_ref(refname).map_err(|err| {
            error!(
                target: "ghostflow/data",
                "failed to delete ref {}: {:?}",
                refname,
                err,
            );
        });
    }
}
