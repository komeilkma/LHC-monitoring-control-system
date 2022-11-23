
use crate::host::{MergeRequest, User};
use crate::utils::Trailer;

/// A trait which represents a filter for trailers to enforce policies on merging.
pub trait MergePolicyFilter {
    /// A method to process trailers and apply policies.
    ///
    /// The `user` parameter is `None` if no user account is associated (or could be found) with
    /// the trailer value.
    fn process_trailer(&mut self, trailer: &Trailer, user: Option<&User>);

    /// The result of the policy.
    ///
    /// The result is either a set of trailers to use for the merge commit message or a list of
    /// reasons the merge is not allowed.
    fn result(self) -> Result<Vec<Trailer>, Vec<String>>;
}

/// A merge policy.
///
/// Merge policies create filters which look at trailers for a merge request and decide what to do
/// with them.
pub trait MergePolicy {
    /// The policy filter type.
    type Filter: MergePolicyFilter;

    /// Create a new policy filter for the given merge request.
    fn for_mr(&self, mr: &MergeRequest) -> Self::Filter;
}

// Merge policies which may be constructed via `Default` can be their own factory.
impl<T> MergePolicy for T
where
    T: MergePolicyFilter + Default,
{
    type Filter = Self;

    fn for_mr(&self, _: &MergeRequest) -> Self::Filter {
        Self::default()
    }
}
