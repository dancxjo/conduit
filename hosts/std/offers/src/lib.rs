//! Exact implementation offers owned by the hosted std Host.

use conduit_core::{
    CapabilityOffer, HostOperationContractId, HostOperationRequirement, SCALAR_ENCODED_LEN,
};
use conduit_std_catalog::{realization_offer, RealizationOfferIdentity};

pub const LOGIC_COMPARE_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-compare-scalar@1";
pub const LOGIC_NOT_IMPLEMENTATION: &str = "std/kernel-logic-not@1";
pub const LOGIC_SELECT_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-select-scalar@1";
pub const MATH_CLAMP_IMPLEMENTATION: &str = "std/kernel-math-clamp-scalar@1";
pub const MATH_SCALE_IMPLEMENTATION: &str = "std/kernel-math-scale-scalar@1";
pub const MATH_DEADBAND_IMPLEMENTATION: &str = "std/kernel-math-deadband-scalar@1";
pub const MATH_CLAMP_HOST_OPERATION: &str = "conduit.host/math-clamp-scalar@1";
pub const MATH_SCALE_HOST_OPERATION: &str = "conduit.host/math-scale-scalar@1";
pub const MATH_DEADBAND_HOST_OPERATION: &str = "conduit.host/math-deadband-scalar@1";

pub fn logic_compare_scalar_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::logic_compare_scalar_contract(),
        conduit_std_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        "logic-compare-scalar-v1",
        "conduit.std/logic-compare-scalar-kernel@1",
        LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-compare-scalar@1",
        None,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::logic_not_contract(),
        conduit_std_catalog::LOGIC_NOT_CONTRACT_REVISION,
        "logic-not-v1",
        "conduit.std/logic-not-kernel@1",
        LOGIC_NOT_IMPLEMENTATION,
        "conduit-std-host/logic-not@1",
        None,
    )
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::logic_select_scalar_contract(),
        conduit_std_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        "logic-select-scalar-v1",
        "conduit.std/logic-select-scalar-kernel@1",
        LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-select-scalar@1",
        None,
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::math_clamp_contract(),
        conduit_std_catalog::MATH_CLAMP_CONTRACT_REVISION,
        "math-clamp-scalar-v1",
        "conduit.std/math-clamp-scalar-kernel@1",
        MATH_CLAMP_IMPLEMENTATION,
        "conduit-std-host/math-clamp-scalar@1",
        Some(MATH_CLAMP_HOST_OPERATION),
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::math_scale_contract(),
        conduit_std_catalog::MATH_SCALE_CONTRACT_REVISION,
        "math-scale-scalar-v1",
        "conduit.std/math-scale-scalar-kernel@1",
        MATH_SCALE_IMPLEMENTATION,
        "conduit-std-host/math-scale-scalar@1",
        Some(MATH_SCALE_HOST_OPERATION),
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    functional_offer(
        conduit_std_catalog::math_deadband_contract(),
        conduit_std_catalog::MATH_DEADBAND_CONTRACT_REVISION,
        "math-deadband-scalar-v1",
        "conduit.std/math-deadband-scalar-kernel@1",
        MATH_DEADBAND_IMPLEMENTATION,
        "conduit-std-host/math-deadband-scalar@1",
        Some(MATH_DEADBAND_HOST_OPERATION),
    )
}

#[allow(clippy::too_many_arguments)]
fn functional_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
    operation: Option<&str>,
) -> CapabilityOffer {
    let target = contract.kind_id.clone();
    let host_operations = operation
        .map(|operation| HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(target),
            maximum_in_flight: 1,
            maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: SCALAR_ENCODED_LEN as u32,
        })
        .into_iter()
        .collect();
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability,
            execution_profile,
            implementation,
            artifact,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    )
}

/// Exact accepted std realization corresponding to every portable nucleus contract.
pub fn supported_nucleus_offers() -> Vec<CapabilityOffer> {
    vec![
        conduit_std_catalog::tick_capability_offer(),
        conduit_std_catalog::time_every_offer(),
        conduit_std_catalog::audio_render_demand_offer(),
        conduit_std_catalog::music_synth_reference_offer(),
        conduit_std_catalog::time_debounce_offer(),
        conduit_std_catalog::time_timeout_offer(),
        conduit_std_catalog::time_delay_offer(),
        conduit_std_catalog::time_throttle_offer(),
        conduit_std_catalog::tick_presentation_offer(),
        conduit_std_catalog::bool_presentation_std_offer(),
        conduit_std_catalog::text_literal_offer(),
        conduit_std_catalog::text_upper_offer(),
        conduit_std_catalog::text_join_offer(),
        conduit_std_catalog::text_presentation_offer(),
        conduit_std_catalog::key_event_tee_offer(),
        conduit_std_catalog::keymap_offer(),
        conduit_std_catalog::chords_offer(),
        conduit_std_catalog::state_count_offer(),
        conduit_std_catalog::state_toggle_offer(),
        conduit_std_catalog::count_presentation_offer(),
        conduit_std_catalog::state_latest_scalar_offer(),
        conduit_std_catalog::flow_tee_scalar_offer(),
        conduit_std_catalog::flow_gate_scalar_offer(),
        conduit_std_catalog::state_select_scalar_offer(),
        logic_compare_scalar_offer(),
        logic_not_offer(),
        logic_select_scalar_offer(),
        math_clamp_offer(),
        math_scale_offer(),
        math_deadband_offer(),
        conduit_std_catalog::layout_viewport_offer(),
        conduit_std_catalog::layout_inset_offer(),
        conduit_std_catalog::layout_row_offer(),
        conduit_std_catalog::layout_column_offer(),
        conduit_std_catalog::layout_stack_offer(),
        conduit_std_catalog::layout_align_offer(),
        conduit_std_catalog::presentation_icon_offer(),
        conduit_std_catalog::presentation_frame_offer(),
        conduit_std_catalog::presentation_badge_offer(),
        conduit_std_catalog::graphics_rect_offer(),
        conduit_std_catalog::graphics_text_offer(),
        conduit_std_catalog::graphics_icon_offer(),
        conduit_std_catalog::graphics_presentation_offer(),
        conduit_std_catalog::bitmap_presentation_offer(),
        conduit_std_catalog::robotics_observe_bump_offer(),
        conduit_std_catalog::robotics_observe_imu_offer(),
        conduit_std_catalog::robotics_observe_range_offer(),
        conduit_std_catalog::robotics_observe_odometry_offer(),
        conduit_std_catalog::robotics_observe_battery_offer(),
        conduit_std_catalog::robotics_velocity_intent_offer(),
        conduit_std_catalog::robotics_drive_differential_offer(),
        conduit_std_catalog::copy_file_offer(),
        conduit_std_catalog::json_encode_std_offer(),
        conduit_std_catalog::json_decode_std_offer(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functional_offers_preserve_exact_portable_contracts() {
        for (offer, contract, revision) in [
            (
                logic_compare_scalar_offer(),
                conduit_std_catalog::logic_compare_scalar_contract(),
                conduit_std_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
            ),
            (
                logic_not_offer(),
                conduit_std_catalog::logic_not_contract(),
                conduit_std_catalog::LOGIC_NOT_CONTRACT_REVISION,
            ),
            (
                logic_select_scalar_offer(),
                conduit_std_catalog::logic_select_scalar_contract(),
                conduit_std_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
            ),
            (
                math_clamp_offer(),
                conduit_std_catalog::math_clamp_contract(),
                conduit_std_catalog::MATH_CLAMP_CONTRACT_REVISION,
            ),
            (
                math_scale_offer(),
                conduit_std_catalog::math_scale_contract(),
                conduit_std_catalog::MATH_SCALE_CONTRACT_REVISION,
            ),
            (
                math_deadband_offer(),
                conduit_std_catalog::math_deadband_contract(),
                conduit_std_catalog::MATH_DEADBAND_CONTRACT_REVISION,
            ),
        ] {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.kind_contract_revision.as_str(), revision);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }

    #[test]
    fn hosted_inventory_matches_every_portable_nucleus_contract_exactly() {
        let contracts = conduit_std_catalog::supported_nucleus_contracts();
        let offers = supported_nucleus_offers();
        assert_eq!(offers.len(), contracts.len());

        for (contract, offer) in contracts.iter().zip(&offers) {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }

    #[test]
    fn neutral_logic_and_math_sources_contain_no_hosted_offer_identity() {
        for source in [
            include_str!("../../../../crates/conduit-std-catalog/src/logic.rs"),
            include_str!("../../../../crates/conduit-std-catalog/src/math.rs"),
        ] {
            for forbidden in [
                "std/kernel-",
                "conduit-std-host/",
                "pub fn logic_compare_scalar_offer",
                "pub fn logic_not_offer",
                "pub fn logic_select_scalar_offer",
                "pub fn math_clamp_offer",
                "pub fn math_scale_offer",
                "pub fn math_deadband_offer",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "neutral source contains {forbidden}"
                );
            }
        }
    }
}
