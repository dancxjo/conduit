use super::*;

#[cfg(feature = "speech")]
#[test]
fn production_realization_families_retain_one_exact_semantic_contract() {
    use conduit_core::{
        resource_requirement, ArtifactId, CapabilityId, CapabilityLimits, ExecutionProfileId,
        HostCallRequirement, ImplementationId, FACE_RESOURCE_CLASS,
    };
    use conduit_face::{renderer_offer, RendererRealizationOffer};

    let renderer = |name: &str| {
        renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(format!("mask-renderer-{name}")),
            execution_profile_id: ExecutionProfileId::from(format!("{name}-profile")),
            implementation_id: ImplementationId::from(format!("{name}-implementation")),
            artifact_id: ArtifactId::from(format!("{name}-artifact")),
            host_call: HostCallRequirement {
                contract_id: conduit_core::HostCallContractId::from(
                    conduit_core::PRESENT_HOST_CALL_CONTRACT,
                ),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_face::MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: conduit_core::MAX_FACE_COMPLETION_BYTES,
            },
            resource_requirement: resource_requirement(FACE_RESOURCE_CLASS, 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: conduit_face::MAX_RENDERER_VALUE_BYTES,
            },
        })
    };
    let families = [
        ("model", vec![model_result_to_text_std_offer()]),
        ("audio", vec![audio_convert_pcm_profile_offer()]),
        ("speech proof", vec![deterministic_speech_offer()]),
        (
            "Mask native/reference",
            vec![renderer("native"), renderer("reference")],
        ),
    ];

    for (family, offers) in families {
        for offer in &offers {
            offer
                .validate_constructed_semantic_contract()
                .unwrap_or_else(|error| panic!("{family} has invalid Kind truth: {error:?}"));
        }
        if let [first, alternate] = offers.as_slice() {
            assert_eq!(first.kind_id, alternate.kind_id, "{family}");
            assert_eq!(
                first.kind_contract_revision, alternate.kind_contract_revision,
                "{family}"
            );
            assert_eq!(first.checked_front(), alternate.checked_front(), "{family}");
            assert_eq!(
                first.semantic_contract, alternate.semantic_contract,
                "{family}"
            );
            assert_ne!(
                first.implementation.implementation_id, alternate.implementation.implementation_id,
                "{family} alternatives must remain distinct Backs"
            );
        }
    }
}

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
    let profile = conduit_semantic_catalog::standard_profile_catalog();
    assert_eq!(offers.len(), contracts.len());

    for (contract, offer) in contracts.iter().zip(&offers) {
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        let checked = profile
            .canonical_kind(&contract.kind_id)
            .expect("portable canonical Kind exists in the standard profile");
        assert_eq!(
            offer.kind_contract_revision,
            checked.kind_contract_revision,
            "{} revision",
            contract.kind_id.as_str()
        );
        assert_eq!(
            offer.semantic_contract,
            checked.semantic_contract(),
            "{} semantic contract",
            contract.kind_id.as_str()
        );
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
        include_str!("../../../../semantics/catalog/src/face_composition.rs"),
        include_str!("../../../../semantics/catalog/src/graphics.rs"),
        include_str!("../../../../semantics/catalog/src/tick_face.rs"),
        include_str!("../../../../semantics/catalog/src/face_bool.rs"),
        include_str!("../../../../semantics/catalog/src/text_face.rs"),
        include_str!("../../../../semantics/catalog/src/graphics_face.rs"),
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
        include_str!("../../../../semantics/catalog/src/structured_music_plot.rs"),
        include_str!("../../../../semantics/catalog/src/robotics.rs"),
        include_str!("../../../../semantics/catalog/src/recurrence_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/calendar_proposal_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/job_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/reminder_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/education_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/vision_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/robotics_structured_catalog.rs"),
        include_str!("../../../../semantics/catalog/src/patchbay_face.rs"),
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
            "pub fn copy_result_face_offer",
            "pub fn structured_literal_std_offer",
            "pub fn structured_face_std_offer",
            "pub fn structured_selector_std_offer",
            "pub fn hosted_keyboard_offer",
            "pub fn generalized_input_std_offers",
            "pub fn tick_capability_offer",
            "pub fn time_every_offer",
            "pub fn time_debounce_offer",
            "pub fn time_timeout_offer",
            "pub fn time_delay_offer",
            "pub fn time_throttle_offer",
            "pub fn time_deadline_offer",
            "pub fn audio_render_demand_offer",
            "pub fn layout_viewport_offer",
            "pub fn layout_inset_offer",
            "pub fn layout_row_offer",
            "pub fn layout_column_offer",
            "pub fn layout_stack_offer",
            "pub fn layout_align_offer",
            "pub fn face_icon_offer",
            "pub fn face_frame_offer",
            "pub fn face_badge_offer",
            "pub fn graphics_rect_offer",
            "pub fn graphics_text_offer",
            "pub fn graphics_icon_offer",
            "pub fn tick_face_offer",
            "pub fn bool_face_std_offer",
            "pub fn bool_face_browser_offer",
            "pub fn text_face_offer",
            "pub fn count_face_offer",
            "pub fn graphics_face_offer",
            "pub fn bitmap_face_offer",
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
            "pub fn scalar_field_face_offer",
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
            "pub fn patchbay_face_offers",
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
            .contains("pub fn count_face_offer")
    );
}
