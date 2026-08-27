//! ConduitOS-owned realization offers for portable standard contracts.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, kind_id,
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
    portable_state_input(
        conduit_std_catalog::state_count_offer(),
        "conduitos-state-count-v1",
        "conduitos/kernel-state-count@1",
    )
}

pub fn state_toggle_offer() -> CapabilityOffer {
    portable_state_input(
        conduit_std_catalog::state_toggle_offer(),
        "conduitos-state-toggle-v1",
        "conduitos/kernel-state-toggle@1",
    )
}

pub fn key_event_tee_offer() -> CapabilityOffer {
    portable_state_input(
        conduit_std_catalog::key_event_tee_offer(),
        "conduitos-key-event-tee-v1",
        "conduitos/kernel-key-event-tee@1",
    )
}

pub fn text_join_offer() -> CapabilityOffer {
    bounded_host_operation(
        conduit_std_catalog::text_join_offer(),
        "conduitos-text-join-v1",
        "conduitos/kernel-text-join@1",
    )
}

pub fn keymap_offer() -> CapabilityOffer {
    bounded_host_operation(
        conduit_std_catalog::keymap_offer(),
        "conduitos-input-keymap-v1",
        "conduitos/kernel-input-keymap@1",
    )
}

pub fn chords_offer() -> CapabilityOffer {
    bounded_host_operation(
        conduit_std_catalog::chords_offer(),
        "conduitos-input-chords-v1",
        "conduitos/kernel-input-chords@1",
    )
}

pub fn time_every_offer() -> CapabilityOffer {
    realize_with(
        conduit_std_catalog::time_every_offer(),
        "conduitos-time-every-v1",
        "conduitos/monotonic-timer-fixed@1",
        "conduitos/kernel-time-every@1",
        "conduitos/time-every@1",
    )
}

pub fn audio_render_demand_offer() -> CapabilityOffer {
    realize_with(
        conduit_std_catalog::audio_render_demand_offer(),
        "conduitos-audio-render-demand-v1",
        "conduitos/monotonic-audio-render-fixed@1",
        "conduitos/kernel-audio-render-demand@1",
        "conduitos/audio-render-demand@1",
    )
}

pub fn time_debounce_offer() -> CapabilityOffer {
    timing(
        conduit_std_catalog::time_debounce_offer(),
        "conduitos/kernel-time-debounce-bool@1",
    )
}

pub fn time_timeout_offer() -> CapabilityOffer {
    timing(
        conduit_std_catalog::time_timeout_offer(),
        "conduitos/kernel-time-timeout-tick-bool@1",
    )
}

pub fn time_delay_offer() -> CapabilityOffer {
    timing(
        conduit_std_catalog::time_delay_offer(),
        "conduitos/kernel-time-delay-bool@1",
    )
}

pub fn time_throttle_offer() -> CapabilityOffer {
    timing(
        conduit_std_catalog::time_throttle_offer(),
        "conduitos/kernel-time-throttle-bool-leading@1",
    )
}

pub fn music_synth_offer() -> CapabilityOffer {
    realize_with(
        conduit_std_catalog::music_synth_reference_offer(),
        "conduitos-music-synth-fixed-q16",
        "conduitos/music-synth-fixed-q16@1",
        "conduitos/kernel-music-synth-fixed-q16@1",
        "conduitos/music-synth-fixed-q16@1",
    )
}

pub fn json_encode_offer() -> CapabilityOffer {
    realize_with(
        conduit_std_catalog::json_encode_std_offer(),
        "conduitos-json-encode-v1",
        "conduitos/fixed-bounded-json@1",
        "conduitos/kernel-json-encode@1",
        "conduit-core/bounded-json@1",
    )
}

pub fn json_decode_offer() -> CapabilityOffer {
    realize_with(
        conduit_std_catalog::json_decode_std_offer(),
        "conduitos-json-decode-v1",
        "conduitos/fixed-bounded-json@1",
        "conduitos/kernel-json-decode@1",
        "conduit-core/bounded-json@1",
    )
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

fn portable_state_input(
    offer: CapabilityOffer,
    capability: &str,
    implementation: &str,
) -> CapabilityOffer {
    realize_with(
        offer,
        capability,
        PORTABLE_STATE_INPUT_PROFILE,
        implementation,
        PORTABLE_STATE_INPUT_ARTIFACT,
    )
}

fn bounded_host_operation(
    offer: CapabilityOffer,
    capability: &str,
    implementation: &str,
) -> CapabilityOffer {
    realize_with(
        offer,
        capability,
        "conduitos/bounded-host-operations@1",
        implementation,
        "conduitos/bounded-host-operations@1",
    )
}

fn timing(offer: CapabilityOffer, implementation: &str) -> CapabilityOffer {
    realize_with(
        offer,
        implementation,
        "conduitos/monotonic-timing-fixed@1",
        implementation,
        "conduitos/timing-nucleus@1",
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
