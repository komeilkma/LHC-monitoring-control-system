#![allow(unused_imports)]
#![allow(clippy::upper_case_acronyms)]

use chrono::Utc;
use graphql_client::GraphQLQuery;
use log::{log, trace, Level};

type DateTime = chrono::DateTime<Utc>;
type GitObjectID = String;
type GitSSHRemote = String;
type URI = String;

#[rustfmt::skip]
macro_rules! gql_query_base {
    ($name:ident) => {
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "src/graphql/schema.graphql",
            query_path = "src/graphql/query.graphql",
            deprecated = "warn",
            response_derives = "Debug, Clone",
            // XXX(graphql_client): Missing space
            variables_derives = "Debug,Clone"
        )]
        pub struct $name;
    };
}

macro_rules! gql_query {
    ($name:ident, $query_name:expr) => {
        gql_query_base!($name);

        impl $name {
            pub(crate) fn name() -> &'static str {
                $query_name
            }
        }
    };
}

macro_rules! gql_mutation {
    ($name:ident, $query_name:expr) => {
        gql_query_base!($name);
    };
}

gql_query!(User, "User");
gql_query!(Commit, "Commit");
gql_query!(PullRequest, "PullRequest");
gql_query!(Repository, "Repository");
gql_query!(IssueID, "IssueID");
gql_query!(PullRequestComments, "PullRequestComments");
gql_query!(PullRequestID, "PullRequestID");
gql_query!(CommitStatuses, "CommitStatuses");
// gql_query!(RepositoryID, "RepositoryID");
gql_query!(PullRequestReactions, "PullRequestReactions");
gql_query!(IssuesClosedByPullRequest, "IssuesClosedByPullRequest");
gql_query!(LabelID, "LabelID");

gql_mutation!(PostComment, "PostComment");
// gql_mutation!(PostCheckRun, "PostCheckRun");
gql_mutation!(AddIssueLabels, "AddIssueLabels");

pub(crate) struct RepoParentInfo<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub ssh_url: &'a str,
    pub parent: Option<(&'a str, &'a str)>,
}

pub(crate) trait RepoInfo {
    fn name(&self) -> String;
    fn ssh_url(&self) -> &str;
    fn parent(&self) -> Option<RepoParentInfo>;
}

macro_rules! impl_repo_info {
    ($type:path) => {
        impl RepoInfo for $type {
            fn name(&self) -> String {
                format!("{}/{}", self.owner.login, self.name)
            }

            fn ssh_url(&self) -> &str {
                &self.ssh_url
            }

            fn parent(&self) -> Option<RepoParentInfo> {
                self.parent.as_ref().map(|parent| {
                    RepoParentInfo {
                        owner: &parent.owner.login,
                        name: &parent.name,
                        ssh_url: &parent.ssh_url.as_ref(),
                        parent: parent.parent.as_ref().map(|grandparent| {
                            (grandparent.owner.login.as_ref(), grandparent.name.as_ref())
                        }),
                    }
                })
            }
        }
    };
}

impl_repo_info!(commit::RepoInfo);
impl_repo_info!(pull_request::RepoInfo);
impl_repo_info!(repository::RepoInfo);
impl_repo_info!(issues_closed_by_pull_request::RepoInfo);

/// Rate limit info for GraphQL queries.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitInfo {
    /// The cost of the query.
    pub cost: i64,
    /// The "credit" limit for the client.
    pub limit: i64,
    /// The number of remaining "credits".
    pub remaining: i64,
    /// When the rate limit resets.
    pub reset_at: DateTime,
}

impl RateLimitInfo {
    pub(crate) fn inspect(&self, name: &str) {
        let (level, desc) = if self.remaining == 0 {
            (Level::Error, "has been hit")
        } else if self.remaining <= 100 {
            (Level::Warn, "is nearing")
        } else if self.remaining <= 1000 {
            (Level::Info, "is approaching")
        } else {
            (Level::Debug, "is OK")
        };

        log!(
            target: "github",
            level,
            "{}: rate limit {}: {} / {} left (resets at {})",
            name, desc, self.remaining, self.limit, self.reset_at,
        );
        trace!(
            target: "github",
            "rate limit cost: {} / {}",
            self.cost,
            self.limit,
        );
    }
}

macro_rules! impl_into_rate_limit_info {
    ($type:path) => {
        impl From<$type> for RateLimitInfo {
            fn from(info: $type) -> Self {
                Self {
                    cost: info.cost,
                    limit: info.limit,
                    remaining: info.remaining,
                    reset_at: info.reset_at,
                }
            }
        }
    };
}

impl_into_rate_limit_info!(user::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(commit::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(pull_request::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(repository::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(issue_id::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(pull_request_comments::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(pull_request_id::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(commit_statuses::RateLimitInfoRateLimit);
//impl_into_rate_limit_info!(repository_id::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(pull_request_reactions::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(issues_closed_by_pull_request::RateLimitInfoRateLimit);
impl_into_rate_limit_info!(label_id::RateLimitInfoRateLimit);
