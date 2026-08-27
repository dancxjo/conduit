use super::*;

#[test]
fn realization_preserves_portable_contract_and_bounds() {
    for (realized, portable) in [
        (
            logic_compare_scalar_offer(),
            conduit_std_catalog::logic_compare_scalar_offer(),
        ),
        (logic_not_offer(), conduit_std_catalog::logic_not_offer()),
        (
            logic_select_scalar_offer(),
            conduit_std_catalog::logic_select_scalar_offer(),
        ),
        (math_clamp_offer(), conduit_std_catalog::math_clamp_offer()),
        (math_scale_offer(), conduit_std_catalog::math_scale_offer()),
        (
            math_deadband_offer(),
            conduit_std_catalog::math_deadband_offer(),
        ),
        (
            state_latest_scalar_offer(),
            conduit_std_catalog::state_latest_scalar_offer(),
        ),
        (
            flow_tee_scalar_offer(),
            conduit_std_catalog::flow_tee_scalar_offer(),
        ),
        (
            state_select_scalar_offer(),
            conduit_std_catalog::state_select_scalar_offer(),
        ),
        (
            flow_gate_scalar_offer(),
            conduit_std_catalog::flow_gate_scalar_offer(),
        ),
        (
            state_count_offer(),
            conduit_std_catalog::state_count_offer(),
        ),
        (
            state_toggle_offer(),
            conduit_std_catalog::state_toggle_offer(),
        ),
        (
            key_event_tee_offer(),
            conduit_std_catalog::key_event_tee_offer(),
        ),
        (text_join_offer(), conduit_std_catalog::text_join_offer()),
        (keymap_offer(), conduit_std_catalog::keymap_offer()),
        (chords_offer(), conduit_std_catalog::chords_offer()),
        (time_every_offer(), conduit_std_catalog::time_every_offer()),
        (
            audio_render_demand_offer(),
            conduit_std_catalog::audio_render_demand_offer(),
        ),
        (
            time_debounce_offer(),
            conduit_std_catalog::time_debounce_offer(),
        ),
        (
            time_timeout_offer(),
            conduit_std_catalog::time_timeout_offer(),
        ),
        (time_delay_offer(), conduit_std_catalog::time_delay_offer()),
        (
            time_throttle_offer(),
            conduit_std_catalog::time_throttle_offer(),
        ),
        (
            music_synth_offer(),
            conduit_std_catalog::music_synth_reference_offer(),
        ),
        (
            json_encode_offer(),
            conduit_std_catalog::json_encode_std_offer(),
        ),
        (
            json_decode_offer(),
            conduit_std_catalog::json_decode_std_offer(),
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
