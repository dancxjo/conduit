//! Installed factory eligibility without adapter-specific realization.
use super::factory;
use conduit_core::PlanFragment;

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
