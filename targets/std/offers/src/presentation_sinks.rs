//! Hosted std manifestations of portable presentation contracts.
pub mod indicator_resource;

use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, CapabilityOffer,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const TICK_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-tick-kernel-hosted@1";
pub const TICK_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-tick@1";
pub const TICK_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-tick@1";
pub const TICK_PRESENTATION_TARGET: &str = "presentation/stdout-tick";

pub const BOOL_PRESENTATION_EXECUTION_PROFILE: &str = "conduit.std/present-bool-kernel-hosted@1";
pub const BOOL_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-present-bool@1";
pub const BOOL_PRESENTATION_ARTIFACT: &str = "conduit-std-host/present-bool@1";
pub const BOOL_PRESENTATION_TARGET: &str = "presentation/stdout-bool";

pub const TEXT_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-text-kernel-hosted@1";
pub const TEXT_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-text@1";
pub const TEXT_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-text@1";
pub const TEXT_PRESENTATION_TARGET: &str = "presentation/stdout-text";

pub const COUNT_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-count-kernel-hosted@1";
pub const COUNT_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-count@1";
pub const COUNT_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-count@1";
pub const COUNT_PRESENTATION_TARGET: &str = "presentation/stdout-count";

pub const GRAPHICS_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-graphics-kernel-hosted@1";
pub const GRAPHICS_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-graphics@1";
pub const GRAPHICS_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-graphics@1";
pub const GRAPHICS_PRESENTATION_TARGET: &str = "presentation/graphics-scene";

pub const BITMAP_PRESENTATION_EXECUTION_PROFILE: &str = "conduit.std/presentation-bitmap-gray8@1";
pub const BITMAP_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-bitmap@1";
pub const BITMAP_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-bitmap@1";
pub const BITMAP_PRESENTATION_TARGET: &str = "presentation/bitmap-gray8";

pub fn tick_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::tick_presentation_contract(),
        conduit_semantic_catalog::TICK_PRESENTATION_CONTRACT_REVISION,
        "presentation-tick-v1",
        TICK_PRESENTATION_EXECUTION_PROFILE,
        TICK_PRESENTATION_IMPLEMENTATION,
        TICK_PRESENTATION_ARTIFACT,
        TICK_PRESENTATION_TARGET,
        conduit_time::TICK_ENCODED_LEN,
    )
}

pub fn bool_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::bool_presentation_contract(),
        conduit_semantic_catalog::BOOL_PRESENTATION_CONTRACT_REVISION,
        "std-bool-presentation-v1",
        BOOL_PRESENTATION_EXECUTION_PROFILE,
        BOOL_PRESENTATION_IMPLEMENTATION,
        BOOL_PRESENTATION_ARTIFACT,
        BOOL_PRESENTATION_TARGET,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn text_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::text_presentation_contract(),
        conduit_semantic_catalog::TEXT_PRESENTATION_CONTRACT_REVISION,
        "presentation-text-v1",
        TEXT_PRESENTATION_EXECUTION_PROFILE,
        TEXT_PRESENTATION_IMPLEMENTATION,
        TEXT_PRESENTATION_ARTIFACT,
        TEXT_PRESENTATION_TARGET,
        conduit_text::MAX_TEXT_BYTES,
    )
}

pub fn count_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::count_presentation_contract(),
        conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
        "presentation-count-v1",
        COUNT_PRESENTATION_EXECUTION_PROFILE,
        COUNT_PRESENTATION_IMPLEMENTATION,
        COUNT_PRESENTATION_ARTIFACT,
        COUNT_PRESENTATION_TARGET,
        conduit_semantic_catalog::COUNT_ENCODED_LEN,
    )
}

pub fn graphics_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::graphics_presentation_contract(),
        conduit_semantic_catalog::GRAPHICS_PRESENTATION_REVISION,
        "presentation-graphics-v1",
        GRAPHICS_PRESENTATION_EXECUTION_PROFILE,
        GRAPHICS_PRESENTATION_IMPLEMENTATION,
        GRAPHICS_PRESENTATION_ARTIFACT,
        GRAPHICS_PRESENTATION_TARGET,
        conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
    )
}

pub fn bitmap_presentation_offer() -> CapabilityOffer {
    presentation_offer(
        conduit_semantic_catalog::bitmap_presentation_contract(),
        conduit_presentation::BITMAP_PRESENTATION_REVISION,
        "presentation-bitmap-gray8-v1",
        BITMAP_PRESENTATION_EXECUTION_PROFILE,
        BITMAP_PRESENTATION_IMPLEMENTATION,
        BITMAP_PRESENTATION_ARTIFACT,
        BITMAP_PRESENTATION_TARGET,
        conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u32,
    )
}

#[allow(clippy::too_many_arguments)]
fn presentation_offer(
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
        vec![present_host_operation_requirement(
            kind_id(target),
            maximum_input_bytes,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
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
                tick_presentation_offer(),
                conduit_semantic_catalog::tick_presentation_contract(),
            ),
            (
                bool_presentation_offer(),
                conduit_semantic_catalog::bool_presentation_contract(),
            ),
            (
                text_presentation_offer(),
                conduit_semantic_catalog::text_presentation_contract(),
            ),
            (
                count_presentation_offer(),
                conduit_semantic_catalog::count_presentation_contract(),
            ),
            (
                graphics_presentation_offer(),
                conduit_semantic_catalog::graphics_presentation_contract(),
            ),
            (
                bitmap_presentation_offer(),
                conduit_semantic_catalog::bitmap_presentation_contract(),
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
        }
    }
}
