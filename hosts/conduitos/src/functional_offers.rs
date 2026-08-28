//! ConduitOS-owned realization offers for portable standard contracts.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, TIMER_RESOURCE_CLASS,
    kind_id, monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement,
    port_id, resource_requirement, wait_host_operation_requirement,
};

pub const FUNCTIONAL_KERNEL_PROFILE: &str = "conduitos/functional-kernel@1";
pub const FUNCTIONAL_KERNEL_ARTIFACT: &str = "conduitos/functional-kernel@1";
pub const LOGIC_COMPARE_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-logic-compare-scalar@1";
pub const LOGIC_NOT_IMPLEMENTATION: &str = "conduitos/kernel-logic-not@1";
pub const LOGIC_SELECT_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-logic-select-scalar@1";
pub const MATH_CLAMP_IMPLEMENTATION: &str = "conduitos/kernel-math-clamp-scalar@1";
pub const FLOW_STATE_PROFILE: &str = "conduitos/flow-state-fixed@1";
pub const FLOW_STATE_ARTIFACT: &str = "conduitos/flow-state@1";
pub const STATE_LATEST_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-state-latest-scalar@1";
pub const FLOW_TEE_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-flow-tee-scalar@1";
pub const STATE_SELECT_SCALAR_IMPLEMENTATION: &str = "conduitos/kernel-state-select-scalar@1";
pub const PORTABLE_STATE_INPUT_PROFILE: &str = "conduitos/portable-state-input-fixed@1";
pub const PORTABLE_STATE_INPUT_ARTIFACT: &str = "conduitos/portable-state-input@1";
pub const ROBOTICS_PROFILE: &str = "conduitos/robotics-prewake-fixed@1";
pub const KEYMAP_HOST_OPERATION: &str = "conduit.host/input-keymap@1";
pub const KEYMAP_HOST_TARGET: &str = "input/keymap-text-fragment";
pub const CHORDS_HOST_OPERATION: &str = "conduit.host/input-chords@1";
pub const CHORDS_HOST_TARGET: &str = "input/chord-fragment";
pub const TEXT_PROFILE: &str = "conduitos/bounded-host-operations@1";
pub const TEXT_ARTIFACT: &str = "conduitos/bounded-host-operations@1";
pub const TEXT_UPPER_HOST_OPERATION: &str = "conduit.host/text-upper@1";
pub const TEXT_UPPER_HOST_OPERATION_TARGET: &str = "text/uppercase-utf8";
pub const TEXT_JOIN_HOST_OPERATION: &str = "conduit.host/text-join@1";
pub const TEXT_JOIN_HOST_OPERATION_TARGET: &str = "text/prefix-concat-utf8";
pub const JSON_ENCODE_HOST_OPERATION: &str = "conduit.host/json-encode@1";
pub const JSON_DECODE_HOST_OPERATION: &str = "conduit.host/json-decode@1";

pub fn logic_compare_scalar_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::logic_compare_scalar_contract(),
            conduit_std_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
            "conduitos/logic-compare-scalar@1",
            LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        ),
        "conduit.host/logic-compare-scalar@1",
        conduit_std_catalog::LOGIC_COMPARE_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn logic_not_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::logic_not_contract(),
            conduit_std_catalog::LOGIC_NOT_CONTRACT_REVISION,
            "conduitos/logic-not@1",
            LOGIC_NOT_IMPLEMENTATION,
        ),
        "conduit.host/logic-not@1",
        conduit_std_catalog::LOGIC_NOT_KIND,
        conduit_core::BOOL_ENCODED_LEN as u32,
        conduit_core::BOOL_ENCODED_LEN as u32,
    )
}

pub fn logic_select_scalar_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::logic_select_scalar_contract(),
            conduit_std_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
            "conduitos/logic-select-scalar@1",
            LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        ),
        "conduit.host/logic-select-scalar@1",
        conduit_std_catalog::LOGIC_SELECT_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::math_clamp_contract(),
            conduit_std_catalog::MATH_CLAMP_CONTRACT_REVISION,
            "conduitos/math-clamp-scalar@1",
            MATH_CLAMP_IMPLEMENTATION,
        ),
        "conduit.host/math-clamp-scalar@1",
        conduit_std_catalog::MATH_CLAMP_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::math_scale_contract(),
            conduit_std_catalog::MATH_SCALE_CONTRACT_REVISION,
            "conduitos-math-scale-scalar-v1",
            "conduitos/kernel-math-scale-scalar@1",
        ),
        "conduit.host/math-scale-scalar@1",
        conduit_std_catalog::MATH_SCALE_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    with_operation(
        realize_contract(
            conduit_std_catalog::math_deadband_contract(),
            conduit_std_catalog::MATH_DEADBAND_CONTRACT_REVISION,
            "conduitos-math-deadband-scalar-v1",
            "conduitos/kernel-math-deadband-scalar@1",
        ),
        "conduit.host/math-deadband-scalar@1",
        conduit_std_catalog::MATH_DEADBAND_KIND,
        conduit_core::SCALAR_ENCODED_LEN as u32,
        conduit_core::SCALAR_ENCODED_LEN as u32,
    )
}

pub fn state_latest_scalar_offer() -> CapabilityOffer {
    realize_flow_contract(
        conduit_std_catalog::state_latest_scalar_contract(),
        conduit_std_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
        "conduitos-state-latest-scalar-v1",
        STATE_LATEST_SCALAR_IMPLEMENTATION,
        Vec::new(),
    )
}

pub fn flow_tee_scalar_offer() -> CapabilityOffer {
    realize_flow_contract(
        conduit_std_catalog::flow_tee_scalar_contract(),
        conduit_std_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
        "conduitos-flow-tee-scalar-v1",
        FLOW_TEE_SCALAR_IMPLEMENTATION,
        Vec::new(),
    )
}

pub fn state_select_scalar_offer() -> CapabilityOffer {
    realize_flow_contract(
        conduit_std_catalog::state_select_scalar_contract(),
        conduit_std_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION,
        "conduitos-state-select-scalar-v1",
        STATE_SELECT_SCALAR_IMPLEMENTATION,
        Vec::new(),
    )
}

pub fn flow_gate_scalar_offer() -> CapabilityOffer {
    realize_flow_contract(
        conduit_std_catalog::flow_gate_scalar_contract(),
        conduit_std_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION,
        "conduitos-flow-gate-scalar-v1",
        "conduitos/kernel-flow-gate-scalar@1",
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from("conduit.host/decode-bool@1"),
            target_kind: Some(kind_id("value/decode-bool")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 1,
        }],
    )
}

pub fn state_count_offer() -> CapabilityOffer {
    realize_state_input_contract(
        conduit_std_catalog::state_count_contract(),
        conduit_std_catalog::STATE_COUNT_CONTRACT_REVISION,
        "conduitos-state-count-v1",
        "conduitos/kernel-state-count@1",
        Vec::new(),
    )
}

pub fn state_toggle_offer() -> CapabilityOffer {
    realize_state_input_contract(
        conduit_std_catalog::state_toggle_contract(),
        conduit_std_catalog::STATE_TOGGLE_CONTRACT_REVISION,
        "conduitos-state-toggle-v1",
        "conduitos/kernel-state-toggle@1",
        Vec::new(),
    )
}

pub fn key_event_tee_offer() -> CapabilityOffer {
    realize_state_input_contract(
        conduit_std_catalog::key_event_tee_contract(),
        conduit_std_catalog::KEY_EVENT_TEE_REVISION,
        "conduitos-key-event-tee-v1",
        "conduitos/kernel-key-event-tee@1",
        Vec::new(),
    )
}

pub fn text_literal_offer() -> CapabilityOffer {
    text_offer(
        conduit_text::text_literal_semantics(),
        "conduitos-text-literal-v1",
        "conduitos/kernel-text-literal@1",
        vec![FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        None,
    )
}

pub fn text_upper_offer() -> CapabilityOffer {
    text_offer(
        conduit_text::text_upper_semantics(),
        "conduitos-text-upper-v1",
        "conduitos/kernel-text-upper@1",
        Vec::new(),
        Some((TEXT_UPPER_HOST_OPERATION, TEXT_UPPER_HOST_OPERATION_TARGET)),
    )
}

pub fn text_join_offer() -> CapabilityOffer {
    text_offer(
        conduit_text::text_join_semantics(),
        "conduitos-text-join-v1",
        "conduitos/kernel-text-join@1",
        vec![FaceStartupParameter {
            name: "prefix".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        Some((TEXT_JOIN_HOST_OPERATION, TEXT_JOIN_HOST_OPERATION_TARGET)),
    )
}

pub fn keymap_offer() -> CapabilityOffer {
    realize_input_host_operation_contract(
        conduit_std_catalog::keymap_contract(),
        conduit_std_catalog::KEYMAP_REVISION,
        "conduitos-input-keymap-v1",
        "conduitos/kernel-input-keymap@1",
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(KEYMAP_HOST_OPERATION),
            target_kind: Some(kind_id(KEYMAP_HOST_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: 4,
        }],
    )
}

pub fn chords_offer() -> CapabilityOffer {
    realize_input_host_operation_contract(
        conduit_std_catalog::chords_contract(),
        conduit_std_catalog::CHORDS_REVISION,
        "conduitos-input-chords-v1",
        "conduitos/kernel-input-chords@1",
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(CHORDS_HOST_OPERATION),
            target_kind: Some(kind_id(CHORDS_HOST_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: conduit_core::CHORD_ENCODED_LEN as u32,
        }],
    )
}

pub fn time_every_offer() -> CapabilityOffer {
    let mut offer = timing_contract(
        conduit_std_catalog::time_every_contract(),
        conduit_time::TIME_EVERY_CONTRACT_REVISION,
        "conduitos-time-every-v1",
        "conduitos/monotonic-timer-fixed@1",
        "conduitos/kernel-time-every@1",
        "conduitos/time-every@1",
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
    );
    offer.startup_parameters[0].value_type = "Duration".into();
    offer.startup_parameters[0].has_default = false;
    offer
}

pub fn tick_offer() -> CapabilityOffer {
    timing_contract(
        conduit_std_catalog::tick_contract(),
        conduit_time::TICK_CONTRACT_REVISION,
        "conduitos-time-tick-v1",
        "conduitos/monotonic-timer-fixed@1",
        "conduitos/kernel-time-tick@1",
        "conduitos/time-tick@1",
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
    )
}

pub fn audio_render_demand_offer() -> CapabilityOffer {
    timing_contract(
        conduit_std_catalog::audio_render_demand_contract(),
        conduit_std_catalog::AUDIO_RENDER_DEMAND_REVISION,
        "conduitos-audio-render-demand-v1",
        "conduitos/monotonic-audio-render-fixed@1",
        "conduitos/kernel-audio-render-demand@1",
        "conduitos/audio-render-demand@1",
        vec![monotonic_timer_host_operation_requirement()],
        vec![monotonic_timer_resource_requirement()],
    )
}

pub fn time_debounce_offer() -> CapabilityOffer {
    timing_offer(
        conduit_std_catalog::time_debounce_contract(),
        conduit_std_catalog::TIME_DEBOUNCE_CONTRACT_REVISION,
        "conduitos/kernel-time-debounce-bool@1",
    )
}

pub fn time_timeout_offer() -> CapabilityOffer {
    timing_offer(
        conduit_std_catalog::time_timeout_contract(),
        conduit_std_catalog::TIME_TIMEOUT_CONTRACT_REVISION,
        "conduitos/kernel-time-timeout-tick-bool@1",
    )
}

pub fn time_delay_offer() -> CapabilityOffer {
    timing_offer(
        conduit_std_catalog::time_delay_contract(),
        conduit_std_catalog::TIME_DELAY_CONTRACT_REVISION,
        "conduitos/kernel-time-delay-bool@1",
    )
}

pub fn time_throttle_offer() -> CapabilityOffer {
    timing_offer(
        conduit_std_catalog::time_throttle_contract(),
        conduit_std_catalog::TIME_THROTTLE_CONTRACT_REVISION,
        "conduitos/kernel-time-throttle-bool-leading@1",
    )
}

pub fn music_synth_offer() -> CapabilityOffer {
    let contract = conduit_std_catalog::music_synth_contract();
    conduit_std_catalog::realization_offer(
        contract,
        conduit_std_catalog::MUSIC_SYNTH_REVISION,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "conduitos-music-synth-fixed-q16",
            execution_profile: "conduitos/music-synth-fixed-q16@1",
            implementation: "conduitos/kernel-music-synth-fixed-q16@1",
            artifact: "conduitos/music-synth-fixed-q16@1",
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(
                "conduit.host/music-synth-render-fixed-q16@1",
            ),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_audio::NOTE_EVENT_ENCODED_LEN
                .max(conduit_audio::CONTROL_EVENT_ENCODED_LEN)
                as u32,
            maximum_output_bytes: conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES,
        }],
        Vec::new(),
        Vec::new(),
    )
}

pub fn json_encode_offer() -> CapabilityOffer {
    json_offer(
        conduit_std_catalog::json_encode_contract(),
        conduit_web::JSON_ENCODE_REVISION,
        "conduitos-json-encode-v1",
        "conduitos/kernel-json-encode@1",
        JSON_ENCODE_HOST_OPERATION,
    )
}

pub fn json_decode_offer() -> CapabilityOffer {
    json_offer(
        conduit_std_catalog::json_decode_contract(),
        conduit_web::JSON_DECODE_REVISION,
        "conduitos-json-decode-v1",
        "conduitos/kernel-json-decode@1",
        JSON_DECODE_HOST_OPERATION,
    )
}

fn json_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
    operation: &str,
) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    let mut offer = conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: "conduitos/fixed-bounded-json@1",
            implementation,
            artifact: "conduit-core/bounded-json@1",
        },
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(target_kind),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
            maximum_output_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
        }],
        Vec::new(),
        Vec::new(),
    );
    offer.shorthand = Some((port_id("value"), port_id("value")));
    offer
}

pub fn robotics_offers() -> Vec<CapabilityOffer> {
    [
        conduit_std_catalog::robotics_observe_bump_offer(),
        conduit_std_catalog::robotics_observe_imu_offer(),
        conduit_std_catalog::robotics_observe_range_offer(),
        conduit_std_catalog::robotics_observe_odometry_offer(),
        conduit_std_catalog::robotics_observe_battery_offer(),
        conduit_std_catalog::robotics_velocity_intent_offer(),
        conduit_std_catalog::robotics_drive_differential_offer(),
    ]
    .into_iter()
    .map(|offer| {
        let slug = offer
            .kind_id
            .as_str()
            .strip_prefix("robotics/")
            .map(String::from)
            .expect("canonical robotics Kind has prefix");
        let revision =
            if offer.kind_id.as_str() == conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND {
                2
            } else {
                1
            };
        realize_with(
            offer,
            &alloc::format!("conduitos-robotics-{slug}@1"),
            ROBOTICS_PROFILE,
            &alloc::format!("conduitos/kernel-robotics-prewake-{slug}@{revision}"),
            "conduitos/robotics-prewake@1",
        )
    })
    .collect()
}

fn realize_state_input_contract(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: PORTABLE_STATE_INPUT_PROFILE,
            implementation,
            artifact: PORTABLE_STATE_INPUT_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    )
}

fn realize_input_host_operation_contract(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: TEXT_PROFILE,
            implementation,
            artifact: TEXT_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    )
}

fn text_offer(
    contract: conduit_text::TextKindContract,
    capability: &str,
    implementation: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    host_operation: Option<(&str, &str)>,
) -> CapabilityOffer {
    let host_operations = host_operation
        .map(|(contract, target)| HostOperationRequirement {
            contract_id: HostOperationContractId::from(contract),
            target_kind: Some(kind_id(target)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        })
        .into_iter()
        .collect();
    let shorthand = (!contract.inputs.is_empty() && !contract.outputs.is_empty())
        .then(|| (port_id("text"), port_id("text")));
    CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TEXT_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(TEXT_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations,
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn timing_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    implementation: &str,
) -> CapabilityOffer {
    let mut offer = timing_contract(
        contract,
        revision,
        implementation,
        "conduitos/monotonic-timing-fixed@1",
        implementation,
        "conduitos/timing-nucleus@1",
        vec![monotonic_timer_host_operation_requirement()],
        vec![monotonic_timer_resource_requirement()],
    );
    offer.startup_parameters[0].value_type = "Duration".into();
    offer
}

#[allow(clippy::too_many_arguments)]
fn timing_contract(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    host_operations: Vec<HostOperationRequirement>,
    resources: Vec<conduit_core::ResourceRequirement>,
) -> CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: profile,
            implementation,
            artifact,
        },
        host_operations,
        resources,
        Vec::new(),
    )
}

fn realize_with(
    mut offer: CapabilityOffer,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    offer.capability_id = CapabilityId::from(capability);
    offer.implementation.execution_profile_id = ExecutionProfileId::from(profile);
    offer.implementation.implementation_id = ImplementationId::from(implementation);
    offer.implementation.artifact_id = ArtifactId::from(artifact);
    offer
}

fn realize_contract(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
) -> CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: FUNCTIONAL_KERNEL_PROFILE,
            implementation,
            artifact: FUNCTIONAL_KERNEL_ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn realize_flow_contract(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    capability: &str,
    implementation: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability,
            execution_profile: FLOW_STATE_PROFILE,
            implementation,
            artifact: FLOW_STATE_ARTIFACT,
        },
        host_operations,
        Vec::new(),
        Vec::new(),
    )
}

fn with_operation(
    mut offer: CapabilityOffer,
    contract: &str,
    target: &str,
    input: u32,
    output: u32,
) -> CapabilityOffer {
    offer.host_operations = vec![HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind_id(target)),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }];
    offer
}

#[cfg(test)]
#[path = "functional_offers_contract_tests.rs"]
mod contract_tests;
