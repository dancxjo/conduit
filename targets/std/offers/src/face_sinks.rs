//! Hosted std manifestations of portable face contracts.
pub mod indicator_resource;

use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, CapabilityOffer,
    FACE_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const TICK_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-tick-kernel-hosted@1";
pub const TICK_FACE_IMPLEMENTATION: &str = "std/kernel-face-tick@1";
pub const TICK_FACE_ARTIFACT: &str = "conduit-std-host/face-tick@1";
pub const TICK_FACE_TARGET: &str = "face/stdout-tick";

pub const BOOL_FACE_EXECUTION_PROFILE: &str = "conduit.std/present-bool-kernel-hosted@1";
pub const BOOL_FACE_IMPLEMENTATION: &str = "std/kernel-present-bool@1";
pub const BOOL_FACE_ARTIFACT: &str = "conduit-std-host/present-bool@1";
pub const BOOL_FACE_TARGET: &str = "face/stdout-bool";

pub const TEXT_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-text-kernel-hosted@1";
pub const TEXT_FACE_IMPLEMENTATION: &str = "std/kernel-face-text@1";
pub const TEXT_FACE_ARTIFACT: &str = "conduit-std-host/face-text@1";
pub const TEXT_FACE_TARGET: &str = "face/stdout-text";

pub const COUNT_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-count-kernel-hosted@1";
pub const COUNT_FACE_IMPLEMENTATION: &str = "std/kernel-face-count@1";
pub const COUNT_FACE_ARTIFACT: &str = "conduit-std-host/face-count@1";
pub const COUNT_FACE_TARGET: &str = "face/stdout-count";

pub const GRAPHICS_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-graphics-kernel-hosted@1";
pub const GRAPHICS_FACE_IMPLEMENTATION: &str = "std/kernel-face-graphics@1";
pub const GRAPHICS_FACE_ARTIFACT: &str = "conduit-std-host/face-graphics@1";
pub const GRAPHICS_FACE_TARGET: &str = "face/graphics-scene";

pub const BITMAP_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-bitmap-gray8@1";
pub const BITMAP_FACE_IMPLEMENTATION: &str = "std/kernel-face-bitmap@1";
pub const BITMAP_FACE_ARTIFACT: &str = "conduit-std-host/face-bitmap@1";
pub const BITMAP_FACE_TARGET: &str = "face/bitmap-gray8";
pub const INDICATOR_FACE_EXECUTION_PROFILE: &str = "conduit.std/face-indicator-kernel-hosted@1";
pub const INDICATOR_FACE_IMPLEMENTATION: &str = "std/kernel-face-indicator@1";
pub const INDICATOR_FACE_ARTIFACT: &str = "conduit-std-host/face-indicator@1";
pub const INDICATOR_FACE_TARGET: &str = "face/stdout-indicator";

pub fn tick_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::tick_face_contract(),
        conduit_semantic_catalog::TICK_FACE_CONTRACT_REVISION,
        "face-tick-v1",
        TICK_FACE_EXECUTION_PROFILE,
        TICK_FACE_IMPLEMENTATION,
        TICK_FACE_ARTIFACT,
        TICK_FACE_TARGET,
        conduit_time::TICK_ENCODED_LEN,
    )
}

pub fn bool_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::bool_face_contract(),
        conduit_semantic_catalog::BOOL_FACE_CONTRACT_REVISION,
        "std-bool-face-v1",
        BOOL_FACE_EXECUTION_PROFILE,
        BOOL_FACE_IMPLEMENTATION,
        BOOL_FACE_ARTIFACT,
        BOOL_FACE_TARGET,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn text_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::text_face_contract(),
        conduit_semantic_catalog::TEXT_FACE_CONTRACT_REVISION,
        "face-text-v1",
        TEXT_FACE_EXECUTION_PROFILE,
        TEXT_FACE_IMPLEMENTATION,
        TEXT_FACE_ARTIFACT,
        TEXT_FACE_TARGET,
        conduit_text::MAX_TEXT_BYTES,
    )
}

pub fn count_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::count_face_contract(),
        conduit_semantic_catalog::COUNT_FACE_CONTRACT_REVISION,
        "face-count-v1",
        COUNT_FACE_EXECUTION_PROFILE,
        COUNT_FACE_IMPLEMENTATION,
        COUNT_FACE_ARTIFACT,
        COUNT_FACE_TARGET,
        conduit_semantic_catalog::COUNT_ENCODED_LEN,
    )
}

pub fn graphics_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::graphics_face_contract(),
        conduit_semantic_catalog::GRAPHICS_FACE_REVISION,
        "face-graphics-v1",
        GRAPHICS_FACE_EXECUTION_PROFILE,
        GRAPHICS_FACE_IMPLEMENTATION,
        GRAPHICS_FACE_ARTIFACT,
        GRAPHICS_FACE_TARGET,
        conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
    )
}

pub fn bitmap_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::bitmap_face_contract(),
        conduit_face::BITMAP_FACE_REVISION,
        "face-bitmap-gray8-v1",
        BITMAP_FACE_EXECUTION_PROFILE,
        BITMAP_FACE_IMPLEMENTATION,
        BITMAP_FACE_ARTIFACT,
        BITMAP_FACE_TARGET,
        conduit_face::MAX_GRAY8_BITMAP_BYTES as u32,
    )
}

pub fn indicator_face_offer() -> CapabilityOffer {
    face_offer(
        conduit_semantic_catalog::indicator_face_contract(),
        conduit_semantic_catalog::INDICATOR_FACE_CONTRACT_REVISION,
        "face-indicator-v1",
        INDICATOR_FACE_EXECUTION_PROFILE,
        INDICATOR_FACE_IMPLEMENTATION,
        INDICATOR_FACE_ARTIFACT,
        INDICATOR_FACE_TARGET,
        conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
    )
}

#[allow(clippy::too_many_arguments)]
fn face_offer(
    contract: StandardKindContract,
    revision: &str,
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
    target: &str,
    maximum_input_bytes: u32,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability,
            execution_profile,
            implementation,
            artifact,
        },
        vec![present_host_call_requirement(
            kind_id(target),
            maximum_input_bytes,
        )],
        vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_std_sink_preserves_the_portable_contract_and_one_bounded_effect() {
        for (offer, contract) in [
            (
                tick_face_offer(),
                conduit_semantic_catalog::tick_face_contract(),
            ),
            (
                bool_face_offer(),
                conduit_semantic_catalog::bool_face_contract(),
            ),
            (
                text_face_offer(),
                conduit_semantic_catalog::text_face_contract(),
            ),
            (
                count_face_offer(),
                conduit_semantic_catalog::count_face_contract(),
            ),
            (
                graphics_face_offer(),
                conduit_semantic_catalog::graphics_face_contract(),
            ),
            (
                bitmap_face_offer(),
                conduit_semantic_catalog::bitmap_face_contract(),
            ),
            (
                indicator_face_offer(),
                conduit_semantic_catalog::indicator_face_contract(),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert_eq!(offer.host_calls.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
        }
    }
}
