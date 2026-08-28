use super::*;

#[test]
fn realization_preserves_portable_contract_and_bounds() {
    for (realized, portable) in [
        (
            logic_compare_scalar_offer(),
            portable_offer(
                conduit_std_catalog::logic_compare_scalar_contract(),
                conduit_std_catalog::LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            logic_not_offer(),
            portable_offer(
                conduit_std_catalog::logic_not_contract(),
                conduit_std_catalog::LOGIC_NOT_CONTRACT_REVISION,
            ),
        ),
        (
            logic_select_scalar_offer(),
            portable_offer(
                conduit_std_catalog::logic_select_scalar_contract(),
                conduit_std_catalog::LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            math_clamp_offer(),
            portable_offer(
                conduit_std_catalog::math_clamp_contract(),
                conduit_std_catalog::MATH_CLAMP_CONTRACT_REVISION,
            ),
        ),
        (
            math_scale_offer(),
            portable_offer(
                conduit_std_catalog::math_scale_contract(),
                conduit_std_catalog::MATH_SCALE_CONTRACT_REVISION,
            ),
        ),
        (
            math_deadband_offer(),
            portable_offer(
                conduit_std_catalog::math_deadband_contract(),
                conduit_std_catalog::MATH_DEADBAND_CONTRACT_REVISION,
            ),
        ),
        (
            state_latest_scalar_offer(),
            portable_offer(
                conduit_std_catalog::state_latest_scalar_contract(),
                conduit_std_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            flow_tee_scalar_offer(),
            portable_offer(
                conduit_std_catalog::flow_tee_scalar_contract(),
                conduit_std_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            state_select_scalar_offer(),
            portable_offer(
                conduit_std_catalog::state_select_scalar_contract(),
                conduit_std_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            flow_gate_scalar_offer(),
            portable_offer(
                conduit_std_catalog::flow_gate_scalar_contract(),
                conduit_std_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION,
            ),
        ),
        (
            state_count_offer(),
            portable_offer(
                conduit_std_catalog::state_count_contract(),
                conduit_std_catalog::STATE_COUNT_CONTRACT_REVISION,
            ),
        ),
        (
            state_toggle_offer(),
            portable_offer(
                conduit_std_catalog::state_toggle_contract(),
                conduit_std_catalog::STATE_TOGGLE_CONTRACT_REVISION,
            ),
        ),
        (
            key_event_tee_offer(),
            portable_offer(
                conduit_std_catalog::key_event_tee_contract(),
                conduit_std_catalog::KEY_EVENT_TEE_REVISION,
            ),
        ),
        (
            text_join_offer(),
            portable_required_text_offer(
                conduit_std_catalog::text_join_contract(),
                conduit_text::TEXT_JOIN_CONTRACT_REVISION,
            ),
        ),
        (
            keymap_offer(),
            portable_offer(
                conduit_std_catalog::keymap_contract(),
                conduit_std_catalog::KEYMAP_REVISION,
            ),
        ),
        (
            chords_offer(),
            portable_offer(
                conduit_std_catalog::chords_contract(),
                conduit_std_catalog::CHORDS_REVISION,
            ),
        ),
        (
            time_every_offer(),
            portable_every_offer(
                conduit_std_catalog::time_every_contract(),
                conduit_time::TIME_EVERY_CONTRACT_REVISION,
            ),
        ),
        (
            audio_render_demand_offer(),
            portable_monotonic_offer(
                conduit_std_catalog::audio_render_demand_contract(),
                conduit_std_catalog::AUDIO_RENDER_DEMAND_REVISION,
                false,
            ),
        ),
        (
            time_debounce_offer(),
            portable_monotonic_offer(
                conduit_std_catalog::time_debounce_contract(),
                conduit_std_catalog::TIME_DEBOUNCE_CONTRACT_REVISION,
                true,
            ),
        ),
        (
            time_timeout_offer(),
            portable_monotonic_offer(
                conduit_std_catalog::time_timeout_contract(),
                conduit_std_catalog::TIME_TIMEOUT_CONTRACT_REVISION,
                true,
            ),
        ),
        (
            time_delay_offer(),
            portable_monotonic_offer(
                conduit_std_catalog::time_delay_contract(),
                conduit_std_catalog::TIME_DELAY_CONTRACT_REVISION,
                true,
            ),
        ),
        (
            time_throttle_offer(),
            portable_monotonic_offer(
                conduit_std_catalog::time_throttle_contract(),
                conduit_std_catalog::TIME_THROTTLE_CONTRACT_REVISION,
                true,
            ),
        ),
        (music_synth_offer(), portable_music_synth_offer()),
        (
            json_encode_offer(),
            portable_json_offer(
                conduit_std_catalog::json_encode_contract(),
                conduit_web::JSON_ENCODE_REVISION,
            ),
        ),
        (
            json_decode_offer(),
            portable_json_offer(
                conduit_std_catalog::json_decode_contract(),
                conduit_web::JSON_DECODE_REVISION,
            ),
        ),
    ] {
        assert_eq!(realized.kind_id, portable.kind_id);
        assert_eq!(
            realized.kind_contract_revision,
            portable.kind_contract_revision
        );
        assert_eq!(realized.inputs, portable.inputs);
        assert_eq!(realized.outputs, portable.outputs);
        assert_eq!(realized.limits, portable.limits);
        assert_eq!(realized.startup_parameters, portable.startup_parameters);
        // Host-operation requirements are realization facts and may be added by
        // ConduitOS; portable semantic and authority facts may not.
        assert_eq!(
            realized.resource_requirements,
            portable.resource_requirements
        );
        assert_eq!(
            realized.authority_requirements,
            portable.authority_requirements
        );
        assert!(realized.capability_id.as_str().contains("conduitos"));
        assert!(
            realized
                .implementation
                .execution_profile_id
                .as_str()
                .starts_with("conduitos/")
        );
        assert!(
            realized
                .implementation
                .implementation_id
                .as_str()
                .starts_with("conduitos/")
        );
    }
}

fn portable_music_synth_offer() -> conduit_core::CapabilityOffer {
    conduit_std_catalog::realization_offer(
        conduit_std_catalog::music_synth_contract(),
        conduit_std_catalog::MUSIC_SYNTH_REVISION,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "proof/music-synth",
            execution_profile: "proof/music-synth@1",
            implementation: "proof/music-synth@1",
            artifact: "proof/music-synth@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                "proof/music-synth-operation@1",
            ),
            target_kind: Some(conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
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

fn portable_json_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
) -> conduit_core::CapabilityOffer {
    let mut offer = conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "proof/portable-json",
            execution_profile: "proof/portable-json",
            implementation: "proof/portable-json",
            artifact: "proof/portable-json",
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    offer.shorthand = Some((
        conduit_core::port_id("value"),
        conduit_core::port_id("value"),
    ));
    offer
}

fn portable_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
) -> conduit_core::CapabilityOffer {
    conduit_std_catalog::realization_offer(
        contract,
        revision,
        conduit_std_catalog::RealizationOfferIdentity {
            capability: "conduitos-test/portable-face",
            execution_profile: "conduitos-test/portable-face@1",
            implementation: "conduitos-test/portable-face@1",
            artifact: "conduitos-test/portable-face@1",
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn portable_required_text_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
) -> conduit_core::CapabilityOffer {
    let mut offer = portable_offer(contract, revision);
    offer.startup_parameters[0].has_default = false;
    offer
}

fn portable_every_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
) -> conduit_core::CapabilityOffer {
    let mut offer = portable_offer(contract, revision);
    offer.startup_parameters[0].value_type = "Duration".into();
    offer.startup_parameters[0].has_default = false;
    offer.resource_requirements = vec![conduit_core::resource_requirement(
        conduit_core::TIMER_RESOURCE_CLASS,
        1,
    )];
    offer
}

fn portable_monotonic_offer(
    contract: conduit_std_catalog::StandardKindContract,
    revision: &str,
    duration_startup: bool,
) -> conduit_core::CapabilityOffer {
    let mut offer = portable_offer(contract, revision);
    if duration_startup {
        offer.startup_parameters[0].value_type = "Duration".into();
    }
    offer.resource_requirements = vec![conduit_core::monotonic_timer_resource_requirement()];
    offer
}

#[test]
fn neutral_catalog_does_not_own_conduitos_realization_identity() {
    for source in [
        include_str!("../../../crates/conduit-std-catalog/src/logic.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/math.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/flow_state.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/state_count.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/state_toggle.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/input_semantics.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/text_transform.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/time_every.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/timing.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/audio_render_demand.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/sound.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/robotics.rs"),
        include_str!("../../../crates/conduit-std-catalog/src/json.rs"),
    ] {
        assert!(!source.contains("conduitos"));
    }
}

#[test]
fn robotics_realizations_preserve_every_portable_contract_and_bound() {
    let realized = robotics_offers();
    let portable = [
        conduit_std_catalog::robotics_observe_bump_offer(),
        conduit_std_catalog::robotics_observe_imu_offer(),
        conduit_std_catalog::robotics_observe_range_offer(),
        conduit_std_catalog::robotics_observe_odometry_offer(),
        conduit_std_catalog::robotics_observe_battery_offer(),
        conduit_std_catalog::robotics_velocity_intent_offer(),
        conduit_std_catalog::robotics_drive_differential_offer(),
    ];
    assert_eq!(realized.len(), portable.len());
    for (realized, portable) in realized.iter().zip(portable) {
        assert_eq!(realized.kind_id, portable.kind_id);
        assert_eq!(
            realized.kind_contract_revision,
            portable.kind_contract_revision
        );
        assert_eq!(realized.inputs, portable.inputs);
        assert_eq!(realized.outputs, portable.outputs);
        assert_eq!(realized.limits, portable.limits);
        assert_eq!(realized.host_operations, portable.host_operations);
        assert_eq!(
            realized.resource_requirements,
            portable.resource_requirements
        );
        assert_eq!(
            realized.authority_requirements,
            portable.authority_requirements
        );
        assert!(
            realized
                .capability_id
                .as_str()
                .starts_with("conduitos-robotics-")
        );
        assert_eq!(
            realized.implementation.execution_profile_id.as_str(),
            ROBOTICS_PROFILE
        );
    }
}
