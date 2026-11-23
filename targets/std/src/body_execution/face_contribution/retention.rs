//! Read-only queries over the route's original private ownership and lifetime.
use super::{Arc, PendingFaceContributionRoute};
impl PendingFaceContributionRoute {
    pub(crate) fn is_retired(&self) -> bool {
        self.scope.is_retired()
    }
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Pending plus transferred fragments on this private scope. Only `stage`
    /// can clone its Arc; observers cannot clone contributions or scope Arcs.
    /// Under the producer's exclusive borrow, other threads can only drop refs,
    /// so a capacity check is conservative even if a drop races this load.
    pub(crate) fn retained_fragments(&self) -> usize {
        Arc::strong_count(&self.scope).saturating_sub(1)
    }
}
