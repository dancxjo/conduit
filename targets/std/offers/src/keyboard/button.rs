//! An explicitly selected Space-key realization of ordinary button meaning.
use conduit_core::{resource_requirement, CapabilityOffer, INPUT_RESOURCE_CLASS};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

pub const IMPLEMENTATION: &str = "std/kernel-space-button@1";
pub const ARTIFACT: &str = "conduit-std-host/space-button@1";
pub const MAPPER: &str = "std/kernel-button-indicator-state@1";
pub const INDICATOR: &str = "std/stdout-indicator-state@1";

pub fn mapper_offer() -> CapabilityOffer {
    realization_offer(
        conduit_semantic_catalog::button_indicator_state_contract(),
        conduit_semantic_catalog::BUTTON_INDICATOR_STATE_REVISION,
        RealizationOfferIdentity {
            capability: MAPPER,
            execution_profile: MAPPER,
            implementation: MAPPER,
            artifact: ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// Current Boolean state manifested through the existing stdout host operation.
pub fn indicator_offer() -> CapabilityOffer {
    let stdout = crate::bool_presentation_offer();
    realization_offer(
        conduit_semantic_catalog::indicator_state_presentation_contract(),
        conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_REVISION,
        RealizationOfferIdentity {
            capability: INDICATOR,
            execution_profile: INDICATOR,
            implementation: INDICATOR,
            artifact: ARTIFACT,
        },
        stdout.host_operations,
        stdout.resource_requirements,
        Vec::new(),
    )
}

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
