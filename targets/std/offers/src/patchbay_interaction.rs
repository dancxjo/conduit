//! Canonical hosted Back for the resident Patchbay input.
use conduit_core::{Back, BackOfferBuilder, CapabilityOffer, HostCallRequirement};

pub const PATCHBAY_INTERACTION_IMPLEMENTATION: &str = "std/patchbay-accept-interaction@1";
pub const PATCHBAY_INTERACTION_OPERATION: &str = "conduit.host/std-patchbay-accept-interaction@1";
const BYTES: u32 = conduit_face::MAX_FACE_INTERACTION_BYTES as u32;

pub fn patchbay_interaction_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        patchbay_application::patchbay_interaction_contract(),
        Back {
            capability_id: PATCHBAY_INTERACTION_IMPLEMENTATION.into(),
            execution_profile_id: "std/bounded-patchbay-interaction@1".into(),
            implementation_id: PATCHBAY_INTERACTION_IMPLEMENTATION.into(),
            artifact_id: "conduit-std-host/patchbay-interaction@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: PATCHBAY_INTERACTION_OPERATION.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
