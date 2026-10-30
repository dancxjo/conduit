//! Borrowed trusted-Make software descriptions for one planning interval.
use crate::PlannerError;
use conduit_core::{CapabilityOffer, HostAdvertisement};

/// Stable complete descriptions; never readiness, membership or effect authority.
///
/// # Safety
/// For the entire borrow, `len` and each successfully decoded complete offer
/// must be deterministic and ordered exactly as the installed Make material.
/// Every field (defaults, laws, realization facts and limits included) must stay
/// unchanged between scans. No incoming wire or mutable callback may establish
/// that contract. The composition root separately retains actual ready owners.
pub unsafe trait IndexedSoftwareOffers {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn decode(&self, index: usize) -> Result<CapabilityOffer, PlannerError>;
}

/// Complete current native facts plus the separately borrowed software set.
/// The combined planning input is not a partial public Host advertisement.
/// Native entries precede software entries, matching ordinary append order.
pub struct IndexedHostOffers<'a> {
    pub native: &'a HostAdvertisement,
    pub software: &'a dyn IndexedSoftwareOffers,
}
