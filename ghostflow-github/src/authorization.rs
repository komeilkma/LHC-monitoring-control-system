use std::collections::BTreeMap;
use std::env;
use std::sync::RwLock;
use std::time;

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use log::error;
use reqwest::blocking::Client;
use reqwest::header::{self, HeaderMap};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ttl_cache::TtlCache;

use crate::client::{GithubError, GithubResult, USER_AGENT};

const LOCK_POISONED: &str = "token lock poisoned";
const TOKEN_SLACK_PERIOD: time::Duration = time::Duration::from_secs(5 * 60);

/// The return type for installation token generation.
#[derive(Deserialize)]
struct InstallationToken {
    token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct Claims {
    iat: i64,
    exp: i64,
    iss: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CurrentUser {
    pub(crate) email: String,
    pub(crate) name: String,
    pub(crate) login: String,
}

#[derive(Debug, Deserialize)]
struct GithubAppUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GithubApp {
    // This field isn't used directly, but is useful from the `impl Debug`
    #[allow(dead_code)]
    id: u64,
    name: String,
    owner: GithubAppUser,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GithubAuthError {
    #[error("jwt error: {}", source)]
    Jwt {
        #[source]
        source: jsonwebtoken::errors::Error,
    },
    #[error("missing installation id: {}", owner)]
    MissingInstallationId { owner: String },
    #[error("key error: {}", source)]
    KeyError {
        #[from]
        source: jsonwebtoken::errors::Error,
    },
    #[error("unknown key format; please file an issue")]
    UnknownKeyFormat {},
}

impl GithubAuthError {
    fn jwt(source: jsonwebtoken::errors::Error) -> Self {
        GithubAuthError::Jwt {
            source,
        }
    }

    fn missing_installation_id(owner: String) -> Self {
        GithubAuthError::MissingInstallationId {
            owner,
        }
    }

    fn unknown_key_format() -> Self {
        Self::UnknownKeyFormat {}
    }
}

type GithubAuthResult<T> = Result<T, GithubAuthError>;

pub(crate) struct GithubAppAuth {
    /// The application ID.
    app_id: i64,
    /// The application private key.
    private_key: EncodingKey,

    /// The base endpoint for REST queries.
    app_endpoint: Url,
    /// The installation ID for each repository owner.
    installation_ids: BTreeMap<String, i64>,
    /// Per-installation tokens for querying GraphQL endpoints.
    tokens: RwLock<TtlCache<String, String>>,
}

impl GithubAppAuth {
    fn jwt(&self) -> GithubAuthResult<String> {
        let header = Header::new(Algorithm::RS256);
        let now = Utc::now();
        let expiration = now + Duration::minutes(10);
        let claims = Claims {
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            iss: self.app_id,
        };
        jsonwebtoken::encode(&header, &claims, &self.private_key).map_err(GithubAuthError::jwt)
    }

    /// Accept headers for the application.
    ///
    /// The application endpoint is a v3 API.
    fn app_accept_headers(&self) -> HeaderMap {
        [
            // GitHub v3 API
            (
                header::ACCEPT,
                "application/vnd.github.v3+json".parse().unwrap(),
            ),
            // GitHub App installations
            (
                header::ACCEPT,
                "application/vnd.github.machine-man-preview+json"
                    .parse()
                    .unwrap(),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    /// Fetch the application information.
    fn app(&self, client: &Client) -> GithubResult<GithubApp> {
        let endpoint = self.app_endpoint.join("app")?;
        let rsp = client
            .get(endpoint.clone())
            .bearer_auth(self.jwt()?)
            .headers(self.app_accept_headers())
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .map_err(|err| GithubError::send_request(endpoint, err))?;
        if !rsp.status().is_success() {
            let err = rsp
                .text()
                .unwrap_or_else(|text_err| format!("failed to extract error body: {:?}", text_err));
            return Err(GithubError::github(err));
        }

        rsp.json().map_err(GithubError::json_response)
    }

    /// Fetch a token for our installation.
    ///
    /// GraphQL requires an installation token in order to access some endpoints. This allows us to
    /// do things which only "bots" are allowed to do (such as submitting status runs for pull
    /// requests).
    fn new_installation_token(
        &self,
        client: &Client,
        owner: &str,
    ) -> GithubResult<(String, time::Duration)> {
        let iid = self
            .installation_ids
            .get(owner)
            .ok_or_else(|| GithubAuthError::missing_installation_id(owner.into()))?;
        let endpoint = self
            .app_endpoint
            .join(&format!("app/installations/{}/access_tokens", iid))?;
        let rsp = client
            .post(endpoint.clone())
            .bearer_auth(self.jwt()?)
            .headers(self.app_accept_headers())
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .map_err(|err| GithubError::send_request(endpoint, err))?;
        if !rsp.status().is_success() {
            let err = rsp
                .text()
                .unwrap_or_else(|text_err| format!("failed to extract error body: {:?}", text_err));
            return Err(GithubError::github(err));
        }

        let rsp: InstallationToken = rsp.json().map_err(GithubError::json_response)?;
        // How log GitHub lets the token live for...
        let now = Utc::now();
        let token_duration = rsp
            .expires_at
            .signed_duration_since(now)
            .to_std()
            .map_err(|_| {
                let msg = format!(
                    "GitHub gave us an expiration time in the past: {} (it is now {})",
                    rsp.expires_at, now,
                );
                error!(target: "github", "{}", msg);
                GithubError::github(msg)
            })?;
        // ...but let's take some time off of it to give us some breathing room.
        let token_duration = token_duration
            .checked_sub(TOKEN_SLACK_PERIOD)
            // Though if that's more time than we have, let's use it while we can.
            .unwrap_or(token_duration);
        Ok((rsp.token, token_duration))
    }

    /// Get the token for GraphQL queries.
    ///
    /// This token can expire, but if it has, it will be refreshed automatically.
    fn token(&self, client: &Client, owner: &str) -> GithubResult<String> {
        // Check for a valid token.
        {
            let lock = self.tokens.read().expect(LOCK_POISONED);
            if let Some(token) = lock.get(owner) {
                return Ok(token.into());
            }
        }

        // No valid token, let's try and get a new one.
        self.refresh_installation_token(client, owner)
    }

    /// Refresh the installation token.
    fn refresh_installation_token(&self, client: &Client, owner: &str) -> GithubResult<String> {
        // Grab an exclusive lock.
        let mut lock = self.tokens.write().expect(LOCK_POISONED);

        // Check if the token is valid again. Multiple queries may have noticed an expired token
        // while one request is fulfilling it. If the token is valid again, use it.
        if let Some(token) = lock.get(owner) {
            return Ok(token.into());
        }

        let (new_token, duration) = self.new_installation_token(client, owner)?;
        // Update the token. The `as_ref_or_update` method cannot be used because
        // `new_installation_token` returns a `Result`.
        lock.insert(owner.into(), new_token.clone(), duration);
        // Return the new token.
        Ok(new_token)
    }
}

impl Clone for GithubAppAuth {
    fn clone(&self) -> Self {
        Self {
            app_id: self.app_id,
            private_key: self.private_key.clone(),
            app_endpoint: self.app_endpoint.clone(),
            installation_ids: self.installation_ids.clone(),
            tokens: RwLock::new(TtlCache::new(self.installation_ids.len())),
        }
    }
}

#[derive(Clone)]
pub(crate) struct GithubActionAuth {
    /// The token for querying GraphQL endpoints.
    token: String,
}

#[derive(Clone)]
pub(crate) enum GithubAuthorization {
    App(Box<GithubAppAuth>),
    Action(GithubActionAuth),
}

impl GithubAuthorization {
    pub(crate) fn new_app(
        host: &str,
        app_id: i64,
        private_key: &[u8],
        installation_ids: BTreeMap<String, i64>,
    ) -> GithubResult<Self> {
        let app_endpoint = Url::parse(&format!("https://{}", host))?;

        // Try to detect a PEM-encoded key (which is what GitHub provides).
        let private_key = if private_key.starts_with(b"-----BEGIN RSA PRIVATE KEY-----") {
            EncodingKey::from_rsa_pem(private_key).map_err(GithubAuthError::from)?
        } else {
            return Err(GithubAuthError::unknown_key_format().into());
        };

        Ok(GithubAuthorization::App(Box::new(GithubAppAuth {
            app_id,
            app_endpoint,
            private_key,
            // Use a dummy token which is invalid right now. It will automatically be
            // refreshed when necessary.
            tokens: RwLock::new(TtlCache::new(installation_ids.len())),
            installation_ids,
        })))
    }

    pub(crate) fn new_action() -> GithubResult<Self> {
        let token = env::var("GITHUB_TOKEN").map_err(GithubError::invalid_token)?;

        Ok(GithubAuthorization::Action(GithubActionAuth {
            token,
        }))
    }

    pub(crate) fn current_user(&self, client: &Client) -> GithubResult<CurrentUser> {
        match *self {
            GithubAuthorization::App(ref auth) => {
                let app = auth.app(client)?;

                Ok(CurrentUser {
                    // TODO(github-enterprise): What email to use here?
                    email: format!("{}@users.noreply.github.com", app.owner.login),
                    login: app.owner.login,
                    name: app.name,
                })
            },
            GithubAuthorization::Action(_) => {
                let login = env::var("GITHUB_ACTOR").map_err(GithubError::invalid_actor)?;

                Ok(CurrentUser {
                    // TODO(github-enterprise): What email to use here?
                    email: format!("{}@users.noreply.github.com", login),
                    login: login.clone(),
                    name: login,
                })
            },
        }
    }

    pub(crate) fn app_id(&self) -> Option<i64> {
        match *self {
            GithubAuthorization::App(ref auth) => Some(auth.app_id),
            GithubAuthorization::Action(_) => None,
        }
    }

    pub(crate) fn token(&self, client: &Client, owner: &str) -> GithubResult<String> {
        match *self {
            GithubAuthorization::App(ref auth) => auth.token(client, owner),
            GithubAuthorization::Action(ref auth) => Ok(auth.token.clone()),
        }
    }
}
