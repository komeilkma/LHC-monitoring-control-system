use std::env;
use std::fmt::Debug;
use std::iter;
use std::thread;
use std::time::Duration;
use graphql_client::{GraphQLQuery, QueryBody, Response};
use itertools::Itertools;
use log::{info, warn};
use reqwest::blocking::Client;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;


// The maximum number of times we will retry server errors.
const BACKOFF_LIMIT: usize = 5;
// The number of seconds to start retries at.
const BACKOFF_START: Duration = Duration::from_secs(1);
// How much to scale retry timeouts for a single query.
const BACKOFF_SCALE: u32 = 2;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GithubError {
    #[error("url parse error: {}", source)]
    UrlParse {
        #[from]
        source: url::ParseError,
    },
    #[error("invalid `GITHUB_TOKEN`: {}", source)]
    InvalidToken {
        #[source]
        source: env::VarError,
    },
    #[error("invalid `GITHUB_ACTOR`: {}", source)]
    InvalidActor {
        #[source]
        source: env::VarError,
    },
    #[error("failed to send request to {}: {}", endpoint, source)]
    SendRequest {
        endpoint: Url,
        #[source]
        source: reqwest::Error,
    },
    #[error("github error: {}", response)]
    Github { response: String },
    #[error("deserialize error: {}", source)]
    Deserialize {
        #[from]
        source: serde_json::Error,
    },
    #[error("github service error: {}", status)]
    GithubService { status: reqwest::StatusCode },
    #[error("json response deserialize: {}", source)]
    JsonResponse {
        #[source]
        source: reqwest::Error,
    },
    #[allow(clippy::upper_case_acronyms)]
    #[error("graphql error: [\"{}\"]", message.iter().format("\", \""))]
    GraphQL { message: Vec<graphql_client::Error> },
    #[error("no response from github")]
    NoResponse {},
    #[error("failure even after exponential backoff")]
    GithubBackoff {},

}

impl GithubError {
    fn should_backoff(&self) -> bool {
        matches!(self, GithubError::GithubService { .. })
    }

    pub(crate) fn send_request(endpoint: Url, source: reqwest::Error) -> Self {
        GithubError::SendRequest {
            endpoint,
            source,
        }
    }

    pub(crate) fn github(response: String) -> Self {
        GithubError::Github {
            response,
        }
    }

    fn github_service(status: reqwest::StatusCode) -> Self {
        GithubError::GithubService {
            status,
        }
    }

    pub(crate) fn json_response(source: reqwest::Error) -> Self {
        GithubError::JsonResponse {
            source,
        }
    }

    pub(crate) fn invalid_token(source: env::VarError) -> Self {
        GithubError::InvalidToken {
            source,
        }
    }

    pub(crate) fn invalid_actor(source: env::VarError) -> Self {
        GithubError::InvalidActor {
            source,
        }
    }

    fn no_response() -> Self {
        GithubError::NoResponse {}
    }

    fn github_backoff() -> Self {
        GithubError::GithubBackoff {}
    }
}

pub(crate) type GithubResult<T> = Result<T, GithubError>;

// The user agent for all queries.
pub(crate) const USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

/// A client for communicating with a Github instance.
#[derive(Clone)]
pub struct Github {
    /// The client used to communicate with Github.
    client: Client,
    /// The endpoint for REST queries.
    rest_endpoint: Url,
    /// The endpoint for GraphQL queries.
    gql_endpoint: Url,
}

    /// Create a new Github client as a GitHub App.
    ///
    /// The `host` parameter is the API endpoint. For example `github.com` uses `api.github.com`.
    ///
    /// The `app_id` and `private_key` are provided when [registering the application][new-app].
    /// The `installation_id` is an ID associated with a given installation of the application. Its
    /// value is present in webhooks, but does not seem to be available generically.
    ///
    /// [new-app]: https://developer.github.com/apps/building-your-first-github-app/#register-a-new-app-with-github
    pub fn new_app<H, P, I, S>(
        host: H,
        app_id: i64,
        private_key: P,
        installation_ids: I,
    ) -> GithubResult<Self>
    where
        H: AsRef<str>,
        P: AsRef<[u8]>,
        I: IntoIterator<Item = (S, i64)>,
        S: Into<String>,
    {
        let ids = installation_ids
            .into_iter()
            .map(|(s, i)| (s.into(), i))
            .collect();
    }

    /// Create a new Github client as a GitHub Action.
    ///
    /// The `host` parameter is the API endpoint. For example `github.com` uses `api.github.com`.
    ///
    /// The `app_id` and `private_key` are provided when [registering the application][new-app].
    /// The `installation_id` is an ID associated with a given installation of the application. Its
    /// value is present in webhooks, but does not seem to be available generically.
    ///
    /// [new-app]: https://developer.github.com/apps/building-your-first-github-app/#register-a-new-app-with-github
    pub fn new_action<H>(host: H) -> GithubResult<Self>
    where
    pub(crate) fn post<D>(&self, owner: &str, endpoint: &str, data: &D) -> GithubResult<Value>
    where
        D: Serialize,
    {
        let endpoint = Url::parse(&format!("{}{}", self.rest_endpoint, endpoint))?;
        let rsp = self
            .client
            .post(endpoint.clone())
            .headers(self.installation_auth_header(owner)?)
            .headers(Self::rest_accept_headers())
            .header(header::USER_AGENT, USER_AGENT)
            .json(data)
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

    /// Send a GraphQL query.
    fn send_impl<Q>(
        &self,
        owner: &str,
        query: &QueryBody<Q::Variables>,
    ) -> GithubResult<Q::ResponseData>
    where
        Q: GraphQLQuery,
        Q::Variables: Debug,
        for<'d> Q::ResponseData: Deserialize<'d>,
    {
        info!(
            target: "github",
            "sending GraphQL query '{}' {:?}",
            query.operation_name,
            query.variables,
        );
        let rsp = self
            .client
            .post(self.gql_endpoint.clone())
            .headers(self.installation_auth_header(owner)?)
            .headers(Self::gql_accept_headers())
            .header(header::USER_AGENT, USER_AGENT)
            .json(query)
            .send()
            .map_err(|err| GithubError::send_request(self.gql_endpoint.clone(), err))?;
        if rsp.status().is_server_error() {
            warn!(
                target: "github",
                "service error {} for query; retrying with backoff",
                rsp.status().as_u16(),
            );
            return Err(GithubError::github_service(rsp.status()));
        }
        if !rsp.status().is_success() {
            let err = rsp
                .text()
                .unwrap_or_else(|text_err| format!("failed to extract error body: {:?}", text_err));
            return Err(GithubError::github(err));
        }

        let rsp: Response<Q::ResponseData> = rsp.json().map_err(GithubError::json_response)?;
        if let Some(errs) = rsp.errors {
            return Err(GithubError::graphql(errs));
        }
        rsp.data.ok_or_else(GithubError::no_response)
    }

    /// Send a GraphQL query.
    pub fn send<Q>(
        &self,
        owner: &str,
        query: &QueryBody<Q::Variables>,
    ) -> GithubResult<Q::ResponseData>
    where
        Q: GraphQLQuery,
        Q::Variables: Debug,
        for<'d> Q::ResponseData: Deserialize<'d>,
    {
        retry_with_backoff(|| self.send_impl::<Q>(owner, query))
    }
}

fn retry_with_backoff<F, K>(mut tryf: F) -> GithubResult<K>
where
    F: FnMut() -> GithubResult<K>,
{
    iter::repeat(())
        .take(BACKOFF_LIMIT)
        .scan(BACKOFF_START, |timeout, _| {
            match tryf() {
                Ok(r) => Some(Some(Ok(r))),
                Err(err) => {
                    if err.should_backoff() {
                        thread::sleep(*timeout);
                        *timeout *= BACKOFF_SCALE;
                        Some(None)
                    } else {
                        Some(Some(Err(err)))
                    }
                },
            }
        })
        .flatten()
        .next()
        .unwrap_or_else(|| Err(GithubError::github_backoff()))
}

#[cfg(test)]
mod test {
    use reqwest::{Client, StatusCode};

    use crate::client::{retry_with_backoff, Github, GithubError, BACKOFF_LIMIT};

    #[test]
    fn backoff_first_success() {
        let mut call_count = 0;
        retry_with_backoff(|| {
            call_count += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(call_count, 1);
    }

    #[test]
    fn backoff_second_success() {
        let mut call_count = 0;
        let mut did_err = false;
        retry_with_backoff(|| {
            call_count += 1;
            if did_err {
                Ok(())
            } else {
                did_err = true;
                Err(GithubError::github_service(
                    StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        })
        .unwrap();
        assert_eq!(call_count, 2);
    }

    #[test]
    fn backoff_no_success() {
        let mut call_count = 0;
        let err = retry_with_backoff::<_, ()>(|| {
            call_count += 1;
            Err(GithubError::github_service(
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        })
        .unwrap_err();
        assert_eq!(call_count, BACKOFF_LIMIT);
        if let GithubError::GithubBackoff {} = err {
        } else {
            panic!("unexpected error: {}", err);
        }
    }

    #[test]
    fn ensure_rest_headers_work() {
        let req = Client::new()
            .post("https://nowhere")
            .headers(Github::rest_accept_headers())
            .build()
            .unwrap();

        let headers = req.headers();

        for (key, value) in Github::rest_accept_headers().iter() {
            if !headers.get_all(key).iter().any(|av| av == value) {
                panic!(
                    "GraphQL request is missing HTTP header `{}: {:?}`",
                    key, value,
                );
            }
        }
    }

}
