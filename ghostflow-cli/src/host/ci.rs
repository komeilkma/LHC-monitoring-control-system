use thiserror::Error;

use crate::host::LocalService;

pub struct Ci;

mod github_action;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CiError {
    #[error("unsupported CI backend: `{}`", backend)]
    UnsupportedCiBackend { backend: String },
    #[error("GitHub action error: {}", source)]
    GithubAction {
        #[from]
        source: github_action::GithubActionError,
    },
}

impl CiError {
    fn unsupported_ci_backend(backend: String) -> Self {
        CiError::UnsupportedCiBackend {
            backend,
        }
    }
}

type CiResult<T> = Result<T, CiError>;

impl Ci {
    pub fn create(backend: &str) -> CiResult<Box<dyn LocalService>> {
        match backend {
            "github-action" => Ok(Box::new(github_action::GithubAction::new()?)),
            _ => Err(CiError::unsupported_ci_backend(backend.into())),
        }
    }
}
