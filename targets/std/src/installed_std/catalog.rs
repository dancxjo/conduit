//! Exact installed factory lookup; the finite registration table has its own owner.
use super::{back::BackFactory, catalog_factories::FACTORIES};
use conduit_core::{ImplementationId, PlanFragment};

pub(super) fn factory(implementation_id: &ImplementationId) -> Option<&'static BackFactory> {
    FACTORIES
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}

pub(crate) fn supports(fragment: &PlanFragment) -> bool {
    !fragment.placements.is_empty()
        && fragment.placements.iter().all(|placement| {
            factory(&placement.implementation_id).is_some()
                || placement.implementation_id.as_str()
                    == conduit_std_offers::STATE_VALUE_STD_IMPLEMENTATION
                || placement.implementation_id.as_str()
                    == conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
        })
}
