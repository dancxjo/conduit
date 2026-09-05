//! Exact implementation offers owned by the hosted std Host.

mod flow_state;
pub use flow_state::*;
mod quantity_mapping;
pub use quantity_mapping::*;
mod quantity_info;
pub use quantity_info::*;
mod pulse_observation;
pub use pulse_observation::*;
mod timing;
pub use timing::*;
mod timed_pattern;
pub use timed_pattern::*;
mod timed_button_attempt;
pub use timed_button_attempt::*;
mod sequence_normalization;
pub use sequence_normalization::*;
mod final_normalized_pattern;
pub use final_normalized_pattern::*;
mod pattern_comparison;
pub use pattern_comparison::*;
mod template_storage;
pub use template_storage::*;
mod presentation_structure;
pub use presentation_structure::*;
mod presentation_sinks;
pub use presentation_sinks::*;
mod text;
pub use text::*;
mod state_input;
pub use state_input::*;
mod alife;
pub use alife::*;
mod json;
pub use json::*;
mod copy_file;
pub use copy_file::*;
mod structured_values;
pub use structured_values::*;
mod image_text;
pub use image_text::*;
mod structured_selector;
pub use structured_selector::*;
mod keyboard;
pub use keyboard::*;
mod generalized_input;
pub use generalized_input::*;
mod music;
pub use music::*;
mod robotics;
pub use robotics::*;
mod calendar;
pub use calendar::*;
mod workflows;
pub use workflows::*;
mod domain_specimens;
pub use domain_specimens::*;
mod patchbay;
pub use patchbay::*;
mod signal;
pub use signal::*;

use conduit_core::{
    CapabilityOffer, HostOperationContractId, HostOperationRequirement, SCALAR_ENCODED_LEN,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity};

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
        conduit_semantic_catalog::logic_compare_scalar_contract(),
        conduit_semantic_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        "logic-compare-scalar-v1",
        "conduit.std/logic-compare-scalar-kernel@1",
        LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-compare-scalar@1",
        None,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::logic_not_contract(),
        conduit_semantic_catalog::LOGIC_NOT_CONTRACT_REVISION,
        "logic-not-v1",
        "conduit.std/logic-not-kernel@1",
        LOGIC_NOT_IMPLEMENTATION,
        "conduit-std-host/logic-not@1",
        None,
    )
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::logic_select_scalar_contract(),
        conduit_semantic_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        "logic-select-scalar-v1",
        "conduit.std/logic-select-scalar-kernel@1",
        LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-select-scalar@1",
        None,
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_clamp_contract(),
        conduit_semantic_catalog::MATH_CLAMP_CONTRACT_REVISION,
        "math-clamp-scalar-v1",
        "conduit.std/math-clamp-scalar-kernel@1",
        MATH_CLAMP_IMPLEMENTATION,
        "conduit-std-host/math-clamp-scalar@1",
        Some(MATH_CLAMP_HOST_OPERATION),
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_scale_contract(),
        conduit_semantic_catalog::MATH_SCALE_CONTRACT_REVISION,
        "math-scale-scalar-v1",
        "conduit.std/math-scale-scalar-kernel@1",
        MATH_SCALE_IMPLEMENTATION,
        "conduit-std-host/math-scale-scalar@1",
        Some(MATH_SCALE_HOST_OPERATION),
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_deadband_contract(),
        conduit_semantic_catalog::MATH_DEADBAND_CONTRACT_REVISION,
        "math-deadband-scalar-v1",
        "conduit.std/math-deadband-scalar-kernel@1",
        MATH_DEADBAND_IMPLEMENTATION,
        "conduit-std-host/math-deadband-scalar@1",
        Some(MATH_DEADBAND_HOST_OPERATION),
    )
}

#[allow(clippy::too_many_arguments)]
fn functional_offer(
    contract: conduit_semantic_catalog::StandardKindContract,
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
        tick_capability_offer(),
        time_every_offer(),
        audio_render_demand_offer(),
        music_synth_reference_offer(),
        time_debounce_offer(),
        time_timeout_offer(),
        time_delay_offer(),
        time_throttle_offer(),
        tick_presentation_offer(),
        bool_presentation_offer(),
        text_literal_offer(),
        text_upper_offer(),
        text_join_offer(),
        text_presentation_offer(),
        key_event_tee_offer(),
        keymap_offer(),
        chords_offer(),
        state_count_offer(),
        state_toggle_offer(),
        count_presentation_offer(),
        state_latest_scalar_offer(),
        flow_tee_scalar_offer(),
        flow_gate_scalar_offer(),
        state_select_scalar_offer(),
        logic_compare_scalar_offer(),
        logic_not_offer(),
        logic_select_scalar_offer(),
        math_clamp_offer(),
        math_scale_offer(),
        math_deadband_offer(),
        quantity_map_offer(),
        quantity_info_offer(),
        layout_viewport_offer(),
        layout_inset_offer(),
        layout_row_offer(),
        layout_column_offer(),
        layout_stack_offer(),
        layout_align_offer(),
        presentation_icon_offer(),
        presentation_frame_offer(),
        presentation_badge_offer(),
        graphics_rect_offer(),
        graphics_text_offer(),
        graphics_icon_offer(),
        graphics_presentation_offer(),
        bitmap_presentation_offer(),
        robotics_observe_bump_offer(),
        robotics_observe_imu_offer(),
        robotics_observe_range_offer(),
        robotics_observe_odometry_offer(),
        robotics_observe_battery_offer(),
        robotics_velocity_intent_offer(),
        robotics_drive_differential_offer(),
        copy_file_offer(),
        json_encode_std_offer(),
        json_decode_std_offer(),
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
                conduit_semantic_catalog::logic_compare_scalar_contract(),
                conduit_semantic_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
            ),
            (
                logic_not_offer(),
                conduit_semantic_catalog::logic_not_contract(),
                conduit_semantic_catalog::LOGIC_NOT_CONTRACT_REVISION,
            ),
            (
                logic_select_scalar_offer(),
                conduit_semantic_catalog::logic_select_scalar_contract(),
                conduit_semantic_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
            ),
            (
                math_clamp_offer(),
                conduit_semantic_catalog::math_clamp_contract(),
                conduit_semantic_catalog::MATH_CLAMP_CONTRACT_REVISION,
            ),
            (
                math_scale_offer(),
                conduit_semantic_catalog::math_scale_contract(),
                conduit_semantic_catalog::MATH_SCALE_CONTRACT_REVISION,
            ),
            (
                math_deadband_offer(),
                conduit_semantic_catalog::math_deadband_contract(),
                conduit_semantic_catalog::MATH_DEADBAND_CONTRACT_REVISION,
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
        let contracts = conduit_semantic_catalog::supported_nucleus_contracts();
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
    fn neutral_sources_contain_no_moved_hosted_offer_identity() {
        for source in [
            include_str!("../../../../semantics/catalog/src/logic.rs"),
            include_str!("../../../../semantics/catalog/src/math.rs"),
            include_str!("../../../../semantics/catalog/src/flow_state.rs"),
            include_str!("../../../../semantics/catalog/src/tick.rs"),
            include_str!("../../../../semantics/catalog/src/time_every.rs"),
            include_str!("../../../../semantics/catalog/src/timing.rs"),
            include_str!("../../../../semantics/catalog/src/audio_render_demand.rs"),
            include_str!("../../../../semantics/catalog/src/layout.rs"),
            include_str!("../../../../semantics/catalog/src/presentation_composition.rs"),
            include_str!("../../../../semantics/catalog/src/graphics.rs"),
            include_str!("../../../../semantics/catalog/src/tick_presentation.rs"),
            include_str!("../../../../semantics/catalog/src/presentation_bool.rs"),
            include_str!("../../../../semantics/catalog/src/text_presentation.rs"),
            include_str!("../../../../semantics/catalog/src/graphics_presentation.rs"),
            include_str!("../../../../semantics/catalog/src/text_transform.rs"),
            include_str!("../../../../semantics/catalog/src/state_count.rs"),
            include_str!("../../../../semantics/catalog/src/state_toggle.rs"),
            include_str!("../../../../semantics/catalog/src/input_semantics.rs"),
            include_str!("../../../../semantics/catalog/src/alife.rs"),
            include_str!("../../../../semantics/catalog/src/json.rs"),
            include_str!("../../../../semantics/catalog/src/copy_file.rs"),
            include_str!("../../../../semantics/catalog/src/structured_values.rs"),
            include_str!("../../../../semantics/catalog/src/structured_selector.rs"),
            include_str!("../../../../semantics/catalog/src/keyboard.rs"),
            include_str!("../../../../semantics/catalog/src/generalized_input_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/sound.rs"),
            include_str!("../../../../semantics/catalog/src/music_input.rs"),
            include_str!("../../../../semantics/catalog/src/structured_music_form.rs"),
            include_str!("../../../../semantics/catalog/src/robotics.rs"),
            include_str!("../../../../semantics/catalog/src/recurrence_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/calendar_proposal_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/job_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/reminder_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/education_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/vision_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/robotics_structured_catalog.rs"),
            include_str!("../../../../semantics/catalog/src/patchbay_presentation.rs"),
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
                "pub fn state_latest_scalar_offer",
                "pub fn flow_tee_scalar_offer",
                "pub fn flow_gate_scalar_offer",
                "pub fn state_select_scalar_offer",
                "pub fn json_encode_std_offer",
                "pub fn json_decode_std_offer",
                "pub fn copy_file_offer",
                "pub fn copy_result_presentation_offer",
                "pub fn structured_literal_std_offer",
                "pub fn structured_presentation_std_offer",
                "pub fn structured_selector_std_offer",
                "pub fn hosted_keyboard_offer",
                "pub fn generalized_input_std_offers",
                "pub fn tick_capability_offer",
                "pub fn time_every_offer",
                "pub fn time_debounce_offer",
                "pub fn time_timeout_offer",
                "pub fn time_delay_offer",
                "pub fn time_throttle_offer",
                "pub fn audio_render_demand_offer",
                "pub fn layout_viewport_offer",
                "pub fn layout_inset_offer",
                "pub fn layout_row_offer",
                "pub fn layout_column_offer",
                "pub fn layout_stack_offer",
                "pub fn layout_align_offer",
                "pub fn presentation_icon_offer",
                "pub fn presentation_frame_offer",
                "pub fn presentation_badge_offer",
                "pub fn graphics_rect_offer",
                "pub fn graphics_text_offer",
                "pub fn graphics_icon_offer",
                "pub fn tick_presentation_offer",
                "pub fn bool_presentation_std_offer",
                "pub fn bool_presentation_browser_offer",
                "pub fn text_presentation_offer",
                "pub fn count_presentation_offer",
                "pub fn graphics_presentation_offer",
                "pub fn bitmap_presentation_offer",
                "pub fn text_literal_offer",
                "pub fn text_upper_offer",
                "pub fn text_join_offer",
                "pub fn state_count_offer",
                "pub fn state_toggle_offer",
                "pub fn key_event_tee_offer",
                "pub fn keymap_offer",
                "pub fn chords_offer",
                "pub fn alife_offers",
                "pub fn orbium_seed_offer",
                "pub fn lenia_step_offer",
                "pub fn scalar_field_presentation_offer",
                "pub fn music_play_midi_offer",
                "pub fn music_synth_reference_offer",
                "pub fn audio_play_alsa_hw_offer",
                "pub fn music_input_midi_offer",
                "pub fn rhythm_compare_std_offer",
                "pub fn instrument_map_std_offer",
                "pub fn robotics_observe_bump_offer",
                "pub fn robotics_observe_imu_offer",
                "pub fn robotics_observe_range_offer",
                "pub fn robotics_observe_odometry_offer",
                "pub fn robotics_observe_battery_offer",
                "pub fn robotics_velocity_intent_offer",
                "pub fn robotics_drive_differential_offer",
                "pub fn recurrence_std_offer",
                "pub fn calendar_proposal_std_offer",
                "pub fn job_std_offers",
                "pub fn reminder_std_offers",
                "pub fn education_std_offers",
                "pub fn vision_std_offers",
                "pub fn robotics_structured_deterministic_offers",
                "pub fn robotics_physical_motion_offer",
                "pub fn patchbay_presentation_offers",
                "std/education-assessment-hosted@1",
                "std/vision-metadata-hosted@1",
                "std/robotics-structured-deterministic@1",
                "patchbay/presenter-kernel-hosted@1",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "neutral source contains {forbidden}"
                );
            }
        }
        assert!(
            !include_str!("../../../../semantics/catalog/src/state_count.rs")
                .contains("pub fn count_presentation_offer")
        );
    }
}
