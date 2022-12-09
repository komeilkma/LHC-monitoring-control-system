// Required to be in the root for `graphql-client`.
mod authorization;
mod client;
pub(crate) mod queries;
pub use crate::client::Github;
pub use crate::client::GithubError;
pub use crate::queries::RateLimitInfo;

