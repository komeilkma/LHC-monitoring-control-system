

//! The `clone` action.
//!
//! This action clones a repository such that it is configured for use by other workflow actions.

use std::borrow::Cow;
use std::collections::hash_map::HashMap;
use std::fmt::{self, Debug};
use std::fs::{create_dir_all, remove_dir_all};
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use git_workarea::{GitContext, GitError};
use log::info;
use thiserror::Error;

use crate::host::{HostedProject, HostingServiceError};

/// Errors which may occur when cloning a remote repository.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CloneError {
    /// The parent directories for the clone action could not be created.
    #[error("failed to create the clone working directory {}: {}", path.display(), source)]
    CreateDirectory {
        /// The path to the directory that could not be created.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to clear old refspecs for fetching.
    #[error("failed to unset all `remote.origin.fetch` settings: {:?}", status)]
    ClearFetchSpecs {
        /// The return code from `git config`.
        status: Option<i32>,
    },
    /// Failure to add add the expected refspecs for fetching.
    #[error(
        "failed to add `remote.origin.fetch` setting for {}: {:?}",
        spec,
        status
    )]
    AddFetchSpec {
        /// The refspec that could not be added.
        spec: String,
        /// The return code from `git config`.
        status: Option<i32>,
    },
    /// Failure to set the `tagopt` setting.
    #[error("failed to set `remote.origin.tagopt: {:?}", status)]
    SetTagopt {
        /// The refspec that could not be added.
        status: Option<i32>,
    },
    /// Failure to initialize a new bare repository.
    #[error("failed to initialize a bare repository in {}: {}", path.display(), output)]
    Initialize {
        /// The path to the bare repository.
        path: PathBuf,
        /// Output from `git init --bare`.
        output: String,
    },
    /// Failure to set the remote repository.
    #[error("failed to set the remote in {} to {}: {}", path.display(), url, output)]
    SetRemote {
        /// The path to the repository.
        path: PathBuf,
        /// The URL of the remote.
        url: String,
        /// Output from `git remote add`.
        output: String,
    },
    /// Failure to set the `logAllRefUpdates` setting.
    #[error("failed to set `core.logAllRefUpdates` in {}: {}", path.display(), output)]
    SetLogRefs {
        /// The path to the repository.
        path: PathBuf,
        /// Output from `git config`.
        output: String,
    },
    /// Failure to remove old submodule directories.
    #[error("failed to remove old submodule directory in {}: {}", path.display(), source)]
    RemoveModuleDirs {
        /// The path to the submodule repository.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to create a submodule directory hierarchy.
    #[error("failed to create submodule directory {}: {}", path.display(), source)]
    CreateModuleDirs {
        /// The path to the submodule directory.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to create a submodule symlink.
    #[error("failed to symlink submodule directory {} -> {}: {}", link.display(), path.display(), source)]
    SymlinkModuleDir {
        /// The path to the symlink.
        link: PathBuf,
        /// The path to the symlink target.
        path: PathBuf,
        /// The source of the error.
        #[source]
        source: io::Error,
    },
    /// Failure to fetch configured refs from the remote.
    #[error("failed to fetch configured refs in {}: {}", path.display(), output)]
    FetchConfigured {
        /// The path to the repository.
        path: PathBuf,
        /// Output from `git fetch`.
        output: String,
    },
    /// Failed to fetch branches from the remote.
    #[error("failed to fetch heads in {}: {}", path.display(), output)]
    FetchHeads {
        /// The path to the repository.
        path: PathBuf,
        /// Output from `git fetch`.
        output: String,
    },
    /// Failure to execute a `git` command.
    #[error("git error: {}", source)]
    Git {
        /// The source of the error.
        #[from]
        source: GitError,
    },
    /// The hosting service returned an error.
    #[error("hosting service error: {}", source)]
    HostingService {
        /// The source of the error.
        #[from]
        source: HostingServiceError,
    },
}

impl CloneError {
    fn create_directory(path: PathBuf, source: io::Error) -> Self {
        CloneError::CreateDirectory {
            path,
            source,
        }
    }

    fn clear_fetch_specs(status: Option<i32>) -> Self {
        CloneError::ClearFetchSpecs {
            status,
        }
    }

    fn add_fetch_spec(spec: String, status: Option<i32>) -> Self {
        CloneError::AddFetchSpec {
            spec,
            status,
        }
    }

    fn set_tagopt(status: Option<i32>) -> Self {
        CloneError::SetTagopt {
            status,
        }
    }

    fn initialize(path: PathBuf, output: &[u8]) -> Self {
        CloneError::Initialize {
            path,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn set_remote(path: PathBuf, url: String, output: &[u8]) -> Self {
        CloneError::SetRemote {
            path,
            url,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn set_log_refs(path: PathBuf, output: &[u8]) -> Self {
        CloneError::SetLogRefs {
            path,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn remove_module_dirs(path: PathBuf, source: io::Error) -> Self {
        CloneError::RemoveModuleDirs {
            path,
            source,
        }
    }

    fn create_module_dirs(path: PathBuf, source: io::Error) -> Self {
        CloneError::CreateModuleDirs {
            path,
            source,
        }
    }

    fn symlink_module_dir(link: PathBuf, path: PathBuf, source: io::Error) -> Self {
        CloneError::SymlinkModuleDir {
            link,
            path,
            source,
        }
    }

    fn fetch_configured(path: PathBuf, output: &[u8]) -> Self {
        CloneError::FetchConfigured {
            path,
            output: String::from_utf8_lossy(output).into(),
        }
    }

    fn fetch_heads(path: PathBuf, output: &[u8]) -> Self {
        CloneError::FetchHeads {
            path,
            output: String::from_utf8_lossy(output).into(),
        }
    }
}

type CloneResult<T> = Result<T, CloneError>;

/// A submodule which should be linked by the clone action from the top-level project.
#[derive(Debug)]
pub enum CloneSubmoduleLink {
    /// A submodule which is hosted under the same work directory as the top-level project.
    Internal(String),
    /// An externally hosted submodule.
    External(PathBuf),
}

impl CloneSubmoduleLink {
    /// Creates a new link.
    ///
    /// Relative paths are assumed to be internal submodules whereas absolute paths are external.
    pub fn new<L>(link: L) -> Self
    where
        L: Into<String>,
    {
        let link = link.into();
        let path = PathBuf::from(&link);

        if path.is_absolute() {
            CloneSubmoduleLink::External(path)
        } else {
            CloneSubmoduleLink::Internal(link)
        }
    }

    /// The path of the git directory.
    fn path<'a>(&'a self, workdir: &Path) -> Cow<'a, Path> {
        match *self {
            CloneSubmoduleLink::External(ref path) => Cow::Borrowed(path),
            CloneSubmoduleLink::Internal(ref link) => {
                Cow::Owned(workdir.join(format!("{}.git", link)))
            },
        }
    }
}

/// A map for submodule paths.
type CloneSubmoduleMap = HashMap<String, CloneSubmoduleLink>;

/// Implementation of the `clone` action.
///
/// Repositories need to be cloned in order for other actions to work. This action bootstraps gets
/// a repository onto the local filesystem and prepared the right way.
pub struct Clone_ {
    /// The path to the working directory for the service.
    ///
    /// All clones are placed underneath this path.
    workdir: PathBuf,
    /// The path to the `.git` directory of the specific clone.
    gitdir: PathBuf,
    /// The project to be cloned.
    project: HostedProject,
    /// Submodules which should be set up for the project.
    submodules: CloneSubmoduleMap,
}

impl Clone_ {
    /// Create a new clone action.
    pub fn new<W: AsRef<Path>>(workdir: W, project: HostedProject) -> Self {
        Self {
            workdir: workdir.as_ref().to_path_buf(),
            gitdir: workdir.as_ref().join(format!("{}.git", project.name)),
            project,
            submodules: CloneSubmoduleMap::new(),
        }
    }

    /// Add a submodule which should be linked to from the clone.
    pub fn with_submodule<N>(&mut self, name: N, submodule: CloneSubmoduleLink) -> &mut Self
    where
        N: Into<String>,
    {
        self.submodules.insert(name.into(), submodule);
        self
    }

    /// Check if the repository is already cloned.
    pub fn exists(&self) -> bool {
        self.gitdir.exists()
    }

    /// Clone a repository which is set up to mirror specific refs of a remote repository.
    pub fn clone_mirror_repo<I, R>(self, refs: I) -> CloneResult<GitContext>
    where
        I: IntoIterator<Item = R>,
        R: AsRef<str>,
    {
        let repo = self.project.service.repo(&self.project.name)?;

        let ctx = self.setup_clone_from(&repo.url)?;

        let clear_fetch = ctx
            .git()
            .arg("config")
            .arg("--unset-all")
            .arg("remote.origin.fetch")
            .status()
            .map_err(|err| GitError::subcommand("config --unset-all remote.origin.fetch", err))?;
        if let Some(5) = clear_fetch.code() {
            // git config --unset return 5 if there were no matches.
        } else if !clear_fetch.success() {
            return Err(CloneError::clear_fetch_specs(clear_fetch.code()));
        }

        for ref_ in refs {
            let refname = ref_.as_ref();
            let refs = ctx
                .git()
                .arg("config")
                .arg("--add")
                .arg("remote.origin.fetch")
                .arg(format!("+{}:{}", refname, refname))
                .status()
                .map_err(|err| GitError::subcommand("config --add remote.origin.fetch", err))?;
            if !refs.success() {
                return Err(CloneError::add_fetch_spec(refname.into(), refs.code()));
            }
        }

        self.setup_submodules(&ctx)?;
        self.fetch_configured(&ctx)?;

        Ok(ctx)
    }

    /// Clone a repository which will be updated manually.
    ///
    /// These repositories should be managed manually, such as triggered by notifications that the
    /// remote repository has been updated or on a timer.
    pub fn clone_watched_repo(self) -> CloneResult<GitContext> {
        let repo = self.project.service.repo(&self.project.name)?;

        let ctx = self.setup_clone_from(&repo.url)?;

        // Tags should not be part of watched repos.
        let no_tags = ctx
            .git()
            .arg("config")
            .arg("remote.origin.tagopt")
            .arg("--no-tags")
            .status()
            .map_err(|err| GitError::subcommand("config remote.origin.tagopt", err))?;
        if !no_tags.success() {
            return Err(CloneError::set_tagopt(no_tags.code()));
        }

        self.setup_submodules(&ctx)?;

        // Fetch the data into the repository.
        self.fetch_heads(&ctx)?;

        Ok(ctx)
    }

    /// Internal method to perform the basic setup of a clone.
    fn setup_clone_from(&self, url: &str) -> CloneResult<GitContext> {
        let ctx = GitContext::new(&self.gitdir);

        if self.exists() {
            return Ok(ctx);
        }

        create_dir_all(ctx.gitdir())
            .map_err(|err| CloneError::create_directory(self.gitdir.clone(), err))?;

        info!(
            target: "ghostflow/clone",
            "cloning from {} into {} for {}",
            url,
            self.gitdir.display(),
            self.project.name,
        );

        let init = ctx
            .git()
            .arg("--bare")
            .arg("init")
            .output()
            .map_err(|err| GitError::subcommand("init", err))?;
        if !init.status.success() {
            return Err(CloneError::initialize(self.gitdir.clone(), &init.stderr));
        }

        // Set the url for the origin remote.
        let remote = ctx
            .git()
            .arg("config")
            .arg("remote.origin.url")
            .arg(url)
            .output()
            .map_err(|err| GitError::subcommand("config remote.origin.url", err))?;
        if !remote.status.success() {
            return Err(CloneError::set_remote(
                self.gitdir.clone(),
                url.into(),
                &remote.stderr,
            ));
        }

        // All ref updates should be logged.
        let log_all_ref_updates = ctx
            .git()
            .arg("config")
            .arg("core.logAllRefUpdates")
            .arg("true")
            .output()
            .map_err(|err| GitError::subcommand("config core.logAllRefUpdates", err))?;
        if !log_all_ref_updates.status.success() {
            return Err(CloneError::set_log_refs(
                self.gitdir.clone(),
                &log_all_ref_updates.stderr,
            ));
        }

        Ok(ctx)
    }

    /// Create symlinks for the submodules of a clone.
    fn setup_submodules(&self, ctx: &GitContext) -> CloneResult<()> {
        let moduledir = ctx.gitdir().join("modules");

        info!(
            target: "ghostflow/clone",
            "removing modules directory: {}",
            moduledir.display(),
        );

        if moduledir.exists() {
            remove_dir_all(&moduledir)
                .map_err(|err| CloneError::remove_module_dirs(moduledir.clone(), err))?;
        }

        for (name, link) in &self.submodules {
            let submodulelink = moduledir.join(name);
            let submoduledir = submodulelink
                .parent()
                .expect("expected the submodule link to have a parent directory");
            let targetdir = link.path(&self.workdir);

            info!(
                target: "ghostflow/clone",
                "linking submodule {}: {} -> {}",
                name,
                submodulelink.display(),
                targetdir.display(),
            );

            create_dir_all(submoduledir)
                .map_err(|err| CloneError::create_module_dirs(submoduledir.into(), err))?;
            symlink(&targetdir, &submodulelink).map_err(|err| {
                CloneError::symlink_module_dir(submodulelink, targetdir.into(), err)
            })?;
        }

        Ok(())
    }

    /// Fetch the default refs from `origin` into the clone.
    fn fetch_configured(&self, ctx: &GitContext) -> CloneResult<()> {
        info!(
            target: "ghostflow/clone",
            "fetching initial pre-configured refs into {}",
            self.gitdir.display(),
        );

        let fetch = ctx
            .git()
            .arg("fetch")
            .arg("origin")
            .output()
            .map_err(|err| GitError::subcommand("fetch", err))?;
        if !fetch.status.success() {
            return Err(CloneError::fetch_configured(
                self.gitdir.clone(),
                &fetch.stderr,
            ));
        }

        Ok(())
    }

    /// Fetch the head refs from `origin` into the clone.
    fn fetch_heads(&self, ctx: &GitContext) -> CloneResult<()> {
        info!(
            target: "ghostflow/clone",
            "fetching initial branch refs into {}",
            self.gitdir.display(),
        );

        let fetch = ctx
            .git()
            .arg("fetch")
            .arg("origin")
            .arg("--prune")
            .arg("+refs/heads/*:refs/heads/*")
            .output()
            .map_err(|err| GitError::subcommand("fetch --prune", err))?;
        if !fetch.status.success() {
            return Err(CloneError::fetch_heads(self.gitdir.clone(), &fetch.stderr));
        }

        Ok(())
    }
}

impl Debug for Clone_ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Clone")
            .field("gitdir", &self.gitdir)
            .field("project", &self.project)
            .field("submodules", &self.submodules)
            .finish()
    }
}
