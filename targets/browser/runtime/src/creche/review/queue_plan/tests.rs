use super::*;
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
};

fn remote_offer(gear: &conduit_plot::CheckedGear) -> CapabilityOffer {
    let slug = gear.kind_id.as_str().replace('/', "-");
    conduit_core::capability_offer_from_parts! {
        semantic_contract: gear.semantic_contract.clone(),
        startup_parameters: gear.startup_parameters.clone(),
        shorthand: gear.shorthand.clone(),
        capability_id: CapabilityId::from(format!("voice-host/{slug}")),
        kind_id: gear.kind_id.clone(),
        kind_contract_revision: gear.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("voice-host/profile@1"),
            implementation_id: ImplementationId::from(format!("voice-host/{slug}@1")),
            artifact_id: ArtifactId::from(format!("voice-host/{slug}-artifact@1")),
        },
        inputs: gear.inputs.clone(),
        outputs: gear.outputs.clone(),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 32,
            max_queue_bytes: 262_144,
        },
    }
}

#[test]
fn spoken_conversation_requires_explicit_audio_authority_and_a_joined_line() {
    let source = include_str!("../../../../../../../plots/live-conversation/main.conduit");
    let (startup, mut profile) = crate::installed_browser::catalogs_for_presentation(
        crate::installed_browser::PresentationProfile::Annotation,
    )
    .unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let selector_offers = crate::installed_browser::catalogs::install_checked_structured_selectors(
        &checked,
        &mut profile,
    )
    .unwrap();
    let backs = crate::installed_browser::backs(&startup, &profile).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_with_backs(
        &checked,
        "spoken-live-conversation",
        &profile,
        &backs,
    )
    .unwrap();
    let mut browser = crate::installed_browser::advertisement(
        HostId::from("browser/orifinia"),
        BootId::from("browser-boot/orifinia"),
    );
    browser.capabilities.extend(selector_offers);
    let remote = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/orifinia-voice"),
        boot_id: BootId::from("boot/orifinia-voice"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("voice-host/profile@1"),
        bases: vec![],
        resources: vec![],
        capabilities: expanded
            .gears
            .iter()
            .filter(|gear| {
                !matches!(
                    gear.kind_id.as_str(),
                    conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND
                        | conduit_semantic_catalog::AUDIO_PLAY_KIND
                )
            })
            .map(remote_offer)
            .collect(),
        planner_capabilities: vec![],
    };
    let hosts = [browser.clone(), remote.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let joined = [crate::creche::JoinedLineObservation {
        host_id: remote.host_id.clone(),
        boot_id: remote.boot_id.clone(),
        carrier: "conduit-line/loopback-websocket@1".into(),
    }];
    let authority = crate::creche::PlanningAuthority {
        browser_audio: true,
    };
    let planned = plan(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        &joined,
        authority,
    )
    .unwrap();

    assert_eq!(planned.fragments.len(), 2);
    assert!(planned.fragments.iter().any(|fragment| fragment
        .connections
        .iter()
        .any(|connection| connection.selected_line.is_some())));
    assert_eq!(
        planned
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.authority.len())
            .sum::<usize>(),
        2
    );
    assert!(plan(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        &joined,
        crate::creche::PlanningAuthority::default(),
    )
    .is_err());
    assert!(plan(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        &[],
        authority,
    )
    .is_err());
}

fn actual_voice_offers() -> Vec<CapabilityOffer> {
    let model = conduit_ai::LocalModelOffer {
        identity: conduit_ai::LocalModelIdentity {
            runtime_name: "fixture".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fixture/build-1".into(),
            model_name: "fixture-model".into(),
            model_content_identity: "sha256-fixture".into(),
            architecture: "fixture".into(),
            parameter_profile: "tiny".into(),
            quantization: "exact".into(),
        },
        limits: conduit_ai::LocalModelLimits {
            work: conduit_ai::LlmWorkBounds::new(4_096, 1, 4_096, 4_096, 0).unwrap(),
            model_bytes: 1,
            admitted_memory_mib: 1,
            compute: conduit_ai::LocalModelComputeNeed {
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 4,
            maximum_queue_bytes: 16_384,
            cancellation_supported: true,
            cache_policy: conduit_ai::LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![conduit_ai::LocalModelKindProfile::StreamGenerate],
        initialized: true,
        lifecycle: conduit_ai::LocalModelLifecycleState::Ready,
        determinism: conduit_ai::LlmDeterminismProfile::ProviderNondeterministic,
    };
    let mut offers = vec![
        conduit_std_offers::speech_window_to_clip_std_offer(),
        conduit_std_offers::whisper_clip_speech_offer(),
        conduit_std_offers::speech_result_to_event_stream_std_offer(),
        conduit_std_offers::recognized_turn_commit_offer(),
        conduit_std_offers::committed_turn_to_text_std_offer(),
        conduit_std_offers::body_conversation_context_std_offer(),
        conduit_std_offers::body_chat_prompt_std_offer(),
        conduit_std_offers::generated_chunk_to_text_std_offer(),
        conduit_std_offers::generated_speech_commit_offer(),
        conduit_std_offers::deterministic_streaming_speech_offer(),
    ];
    offers.extend(model.capability_offers().unwrap());
    offers
}

#[test]
fn reviewed_fixture_voice_offers_cover_every_expanded_remote_conversation_gear() {
    let source = include_str!("../../../../../../../plots/live-conversation/main.conduit");
    let (startup, mut profile) = crate::installed_browser::catalogs_for_presentation(
        crate::installed_browser::PresentationProfile::Annotation,
    )
    .unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    crate::installed_browser::catalogs::install_checked_structured_selectors(
        &checked,
        &mut profile,
    )
    .unwrap();
    let backs = crate::installed_browser::backs(&startup, &profile).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_with_backs(
        &checked,
        "spoken-live-conversation",
        &profile,
        &backs,
    )
    .unwrap();
    let offers = actual_voice_offers();

    for gear in expanded.gears.iter().filter(|gear| {
        !matches!(
            gear.kind_id.as_str(),
            conduit_semantic_catalog::AUDIO_CAPTURE_PUSH_TO_TALK_KIND
                | conduit_semantic_catalog::AUDIO_PLAY_KIND
        )
    }) {
        assert!(
            offers
                .iter()
                .any(|offer| offer.checked_front() == gear.checked_front()),
            "real Voice Host offers do not realize expanded Gear {}",
            gear.kind_id.as_str()
        );
    }

    for required_adapter in [
        conduit_tongues::SPEECH_WINDOW_TO_CLIP_KIND,
        conduit_tongues::SPEECH_RECOGNIZE_CLIP_KIND,
        conduit_tongues::SPEECH_RESULT_TO_EVENT_STREAM_KIND,
    ] {
        assert!(
            expanded
                .gears
                .iter()
                .any(|gear| gear.kind_id.as_str() == required_adapter),
            "streaming recognition Back hid adapter {required_adapter}"
        );
    }
}
