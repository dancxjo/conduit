//! An explicitly selected Space-key realization of ordinary button meaning.
use conduit_core::{resource_requirement, CapabilityOffer, INPUT_RESOURCE_CLASS};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const IMPLEMENTATION: &str = "std/kernel-space-button@1";
pub const ARTIFACT: &str = "conduit-std-host/space-button@1";

/// Advertise only with a currently acquired keyboard input resource/adapter.
pub fn offer() -> CapabilityOffer {
    realization_offer(
        conduit_semantic_catalog::button_source_contract(),
        conduit_semantic_catalog::BUTTON_SOURCE_REVISION,
        RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![super::next_key_event_host_operation_requirement()],
        vec![resource_requirement(INPUT_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}
