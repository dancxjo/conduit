//! Separately admitted consumption of committed producer facts.
use super::PublicationSnapshot;
use core::task::{Context, Poll};

/// A bounded consumer of one committed source publication. Pending must register
/// the supplied waker for its actual readiness; it does not authorize polling,
/// retries, storage effects or new work. The producer keeps the exact snapshot
/// stable and advances no further kernel Step until Ready or cancellation.
/// Ready(Ok) acknowledges this sink's acceptance contract, not durable storage
/// unless that contract actually verifies the selected storage receipt. Source
/// time/context must be retained independently; delayed receipt time is not a
/// substitute acquisition or producer observation time.
pub trait PublicationSink {
    fn poll_publication(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), conduit_kernel::Failure>>;
}
impl<T: PublicationSink + ?Sized> PublicationSink for &mut T {
    fn poll_publication(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), conduit_kernel::Failure>> {
        (**self).poll_publication(snapshot, context)
    }
}
