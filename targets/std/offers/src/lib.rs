//! Exact implementation offers owned by the hosted std Host.

mod value_pair;
pub use value_pair::*;

mod flow_state;
pub use flow_state::*;
mod data_text;
pub use data_text::*;
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
mod face_structure;
pub use face_structure::*;
mod face_sinks;
pub use face_sinks::*;
mod text;
pub use text::*;
mod morse_composition;
pub use morse_composition::*;
mod text_state;
pub use text_state::*;
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
mod current_sample;
pub use current_sample::*;
mod combine_latest;
pub use combine_latest::*;
mod thermostat;
pub use thermostat::*;
mod todo;
pub use todo::*;
mod flow_zip;
pub use flow_zip::*;
mod flow_collect;
pub use flow_collect::*;
mod flow_join_by_key;
pub use flow_join_by_key::*;
mod image_text;
pub use image_text::*;
mod typed_record;
pub use typed_record::*;
mod record_temporal;
pub use record_temporal::*;
mod record_queue;
pub use record_queue::*;
mod record_delivery;
pub use record_delivery::*;
mod record_transcript;
pub use record_transcript::*;
mod structured_selector;
pub use structured_selector::*;
mod pure_expression;
pub use pure_expression::*;
mod keyboard;
pub use keyboard::*;
mod generalized_input;
pub use generalized_input::*;
mod music;
pub use music::*;
mod robotics;
pub use robotics::*;
mod navigation;
pub use navigation::*;
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
#[cfg(feature = "speech")]
mod house_conversation;
#[cfg(feature = "speech")]
pub use house_conversation::*;
mod body_chat;
pub use body_chat::*;
mod model_text;
pub use model_text::*;
#[cfg(feature = "speech")]
mod speech_recognition;
#[cfg(feature = "speech")]
pub use speech_recognition::*;
#[cfg(feature = "speech")]
mod speech_recognition_adapters;
#[cfg(feature = "speech")]
pub use speech_recognition_adapters::*;
#[cfg(feature = "speech")]
mod speech_commit;
#[cfg(feature = "speech")]
pub use speech_commit::*;
#[cfg(feature = "speech")]
mod espeak_speech;
#[cfg(feature = "speech")]
pub use espeak_speech::*;
#[cfg(feature = "speech")]
mod speech_synthesis;
#[cfg(feature = "speech")]
pub use speech_synthesis::*;
#[cfg(feature = "speech")]
mod spoken_mask;
#[cfg(feature = "speech")]
pub use spoken_mask::*;
#[cfg(feature = "speech")]
mod conversation_commit;
#[cfg(feature = "speech")]
pub use conversation_commit::*;
mod microphone;
pub use microphone::*;
mod vision;
pub use vision::*;

use conduit_core::{CapabilityOffer, HostCallContractId, HostCallRequirement, SCALAR_ENCODED_LEN};

pub const LOGIC_COMPARE_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-compare-scalar@1";
pub const LOGIC_NOT_IMPLEMENTATION: &str = "std/kernel-logic-not@1";
pub const LOGIC_SELECT_SCALAR_IMPLEMENTATION: &str = "std/kernel-logic-select-scalar@1";
pub const MATH_CLAMP_IMPLEMENTATION: &str = "std/kernel-math-clamp-scalar@1";
pub const MATH_SCALE_IMPLEMENTATION: &str = "std/kernel-math-scale-scalar@1";
pub const MATH_DEADBAND_IMPLEMENTATION: &str = "std/kernel-math-deadband-scalar@1";
pub const MATH_CLAMP_HOST_CALL: &str = "conduit.host/math-clamp-scalar@1";
pub const MATH_SCALE_HOST_CALL: &str = "conduit.host/math-scale-scalar@1";
pub const MATH_DEADBAND_HOST_CALL: &str = "conduit.host/math-deadband-scalar@1";

pub fn logic_compare_scalar_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::logic_compare_scalar_contract().into_semantic_contract(
            conduit_semantic_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        ),
        "logic-compare-scalar-v1",
        "conduit.std/logic-compare-scalar-kernel@1",
        LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-compare-scalar@1",
        None,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::logic_not_contract()
            .into_semantic_contract(conduit_semantic_catalog::LOGIC_NOT_CONTRACT_REVISION),
        "logic-not-v1",
        "conduit.std/logic-not-kernel@1",
        LOGIC_NOT_IMPLEMENTATION,
        "conduit-std-host/logic-not@1",
        None,
    )
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::logic_select_scalar_contract().into_semantic_contract(
            conduit_semantic_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        ),
        "logic-select-scalar-v1",
        "conduit.std/logic-select-scalar-kernel@1",
        LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        "conduit-std-host/logic-select-scalar@1",
        None,
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_clamp_semantic_contract(),
        "math-clamp-scalar-v1",
        "conduit.std/math-clamp-scalar-kernel@1",
        MATH_CLAMP_IMPLEMENTATION,
        "conduit-std-host/math-clamp-scalar@1",
        Some(MATH_CLAMP_HOST_CALL),
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_scale_semantic_contract(),
        "math-scale-scalar-v1",
        "conduit.std/math-scale-scalar-kernel@1",
        MATH_SCALE_IMPLEMENTATION,
        "conduit-std-host/math-scale-scalar@1",
        Some(MATH_SCALE_HOST_CALL),
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    functional_offer(
        conduit_semantic_catalog::math_deadband_semantic_contract(),
        "math-deadband-scalar-v1",
        "conduit.std/math-deadband-scalar-kernel@1",
        MATH_DEADBAND_IMPLEMENTATION,
        "conduit-std-host/math-deadband-scalar@1",
        Some(MATH_DEADBAND_HOST_CALL),
    )
}

#[allow(clippy::too_many_arguments)]
fn functional_offer(
    contract: conduit_core::Kind,
    capability: &str,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
    operation: Option<&str>,
) -> CapabilityOffer {
    let target = contract.kind_id.clone();
    let host_calls = operation
        .map(|operation| HostCallRequirement {
            contract_id: HostCallContractId::from(operation),
            target_kind: Some(target),
            maximum_in_flight: 1,
            maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: SCALAR_ENCODED_LEN as u32,
        })
        .into_iter()
        .collect();
    conduit_core::BackOfferBuilder::new(
        contract,
        conduit_core::Back {
            capability_id: capability.into(),
            execution_profile_id: execution_profile.into(),
            implementation_id: implementation.into(),
            artifact_id: artifact.into(),
            host_calls,
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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
        time_deadline_offer(),
        tick_face_offer(),
        bool_face_offer(),
        text_literal_offer(),
        text_upper_offer(),
        text_join_offer(),
        text_face_offer(),
        key_event_tee_offer(),
        keymap_offer(),
        chords_offer(),
        state_count_offer(),
        state_toggle_offer(),
        count_face_offer(),
        state_latest_scalar_offer(),
        flow_tee_scalar_offer(),
        flow_first_scalar_offer(),
        flow_gate_scalar_offer(),
        state_select_scalar_offer(),
        logic_compare_scalar_offer(),
        logic_not_offer(),
        logic_select_scalar_offer(),
        math_clamp_offer(),
        math_scale_offer(),
        math_deadband_offer(),
        quantity_map_offer(),
        distance_frequency_map_offer(),
        audio_time_analysis_offer(),
        audio_spectral_analysis_offer(),
        audio_pitch_analysis_offer(),
        quantity_info_offer(),
        layout_viewport_offer(),
        layout_inset_offer(),
        layout_row_offer(),
        layout_column_offer(),
        layout_stack_offer(),
        layout_align_offer(),
        face_icon_offer(),
        face_frame_offer(),
        face_badge_offer(),
        graphics_rect_offer(),
        graphics_text_offer(),
        graphics_icon_offer(),
        graphics_face_offer(),
        bitmap_face_offer(),
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
        audio_tone_offer(),
    ]
}

#[cfg(test)]
mod tests;

mod audio_analysis;
pub use audio_analysis::*;

mod pcm_packing;
pub use pcm_packing::*;
