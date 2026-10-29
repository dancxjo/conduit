use conduit_core::BaseImplementationId;
/// Closed-world policy for Cord realization mechanisms.
///
/// This is deliberately unrelated to `HostAdvertisement::bases`: advertised
/// Base providers own capability/resource provenance, while this policy only
/// permits an in-host Cord or one exact remotely offered Line mechanism.
#[derive(Clone, Copy)]
pub(crate) struct LineMechanismPolicy<'a> {
    allowed: &'a [BaseImplementationId],
}

impl<'a> LineMechanismPolicy<'a> {
    pub(crate) const fn new(allowed: &'a [BaseImplementationId]) -> Self {
        Self { allowed }
    }

    pub(crate) fn permits_local(self) -> bool {
        self.allowed
            .iter()
            .any(|id| id.as_str() == conduit_core::LOCAL_BASE_IMPLEMENTATION_ID)
    }

    pub(crate) fn permits_remote(self, implementation: &BaseImplementationId) -> bool {
        implementation.as_str() != conduit_core::LOCAL_BASE_IMPLEMENTATION_ID
            && self.allowed.contains(implementation)
    }
}
