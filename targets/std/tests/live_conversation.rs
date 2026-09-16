//! Canonical live-conversation Form conformance across semantic owners.

use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LineId, LinkBindingId, LinkEndpointId, OfferGeneration, PortTemporal, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindDefinition, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup
        .insert_value_kind_alias(
            "PcmFrames",
            conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        )
        .unwrap();
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
    conduit_tongues::install_speech_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn canonical_live_conversation_is_one_reviewed_temporal_form() {
    let source = include_str!("../../../forms/live-conversation/main.conduit");
    let (startup, profile) = catalogs();

    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "live-conversation", &profile).unwrap();
    assert_eq!(
        authored.face.inputs()[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(
        authored.face.outputs()[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    let kinds = authored
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    for expected in [
        conduit_tongues::STREAMING_SPEECH_RECOGNIZE_KIND,
        conduit_tongues::COMMIT_RECOGNIZED_TURN_KIND,
        conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND,
        conduit_chat::BODY_CONVERSATION_CONTEXT_KIND,
        conduit_chat::BODY_CHAT_PROMPT_KIND,
        conduit_ai::LLM_STREAM_GENERATE_KIND,
        conduit_ai::GENERATED_CHUNK_TO_TEXT_KIND,
        conduit_tongues::SPEECH_COMMIT_KIND,
        conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND,
    ] {
        assert!(
            kinds.contains(&expected),
            "canonical Form omitted {expected}"
        );
    }
    let lower = source.to_ascii_lowercase();
    for forbidden in [
        "whisper",
        "piper",
        "ollama",
        "alsa",
        "webaudio",
        "browser",
        "host",
        "socket",
        "conduit-test",
        "wav",
    ] {
        assert!(
            !lower.contains(forbidden),
            "canonical Form contains {forbidden}"
        );
    }
}

fn synthetic_offer(definition: &KindDefinition, host: &str) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    CapabilityOffer {
        startup_parameters: definition
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.default_value {
                    ConfigurationValue::Bool(_) => "Boolean",
                    ConfigurationValue::I64(_) => "Scalar",
                    ConfigurationValue::U64(_) => "Count",
                    ConfigurationValue::Text(_) => "Text",
                    ConfigurationValue::Structured(ref value) => value.profile().as_str(),
                }
                .into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("proof-{host}-{slug}")),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/live-conversation@1"),
            implementation_id: ImplementationId::from(format!("proof/{host}/{slug}@1")),
            artifact_id: ArtifactId::from(format!("proof/{host}/{slug}-artifact@1")),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 32,
            max_queue_bytes: 262_144,
        },
    }
}

fn proof_host(name: &str, definitions: &[&KindDefinition]) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/live-{name}")),
        boot_id: BootId::from(format!("boot/live-{name}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("proof/live-conversation-host@1"),
        resources: vec![],
        capabilities: definitions
            .iter()
            .map(|definition| synthetic_offer(definition, name))
            .collect(),
        planner_capabilities: vec![],
    }
}

fn proof_line(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    suffix: &str,
) -> conduit_core::LineOffer {
    let mut line = conduit_signal_conformance::distributed_websocket_line_offer();
    line.line_id = LineId::from(format!("live-conversation/{suffix}"));
    line.binding.binding_id = LinkBindingId::from(format!("live-conversation/{suffix}/binding"));
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.source.endpoint_id = LinkEndpointId::from(format!("{suffix}/egress"));
    line.binding.sink.host_id = sink.host_id.clone();
    line.binding.sink.boot_id = sink.boot_id.clone();
    line.binding.sink.endpoint_id = LinkEndpointId::from(format!("{suffix}/ingress"));
    line.binding.limits.maximum_in_flight_items = 32;
    line.binding.limits.maximum_payload_bytes = 262_144;
    line.binding.limits.maximum_buffered_bytes = 262_144;
    line.binding.limits.maximum_frame_bytes = 262_144;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.availability.sign_id = SignId::from(format!("live-conversation/{suffix}/ready"));
    line
}

#[test]
fn unchanged_live_conversation_source_plans_across_compatible_hosts() {
    let source = include_str!("../../../forms/live-conversation/main.conduit");
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "live-conversation", &profile).unwrap();
    let definitions = authored
        .expanded
        .gears
        .iter()
        .map(|gear| profile.get(&gear.kind_id).unwrap())
        .collect::<Vec<_>>();
    let input = proof_host("input", &definitions);
    let response = proof_host("response", &definitions);
    let hosts = [input.clone(), response.clone()];
    let placements = PlacementChoices {
        by_gear: authored
            .expanded
            .gears
            .iter()
            .enumerate()
            .map(|(index, gear)| {
                let host = if index < 4 { &input } else { &response };
                let capability = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.checked_face() == gear.checked_face())
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let lines = [
        proof_line(&input, &response, "input-to-response"),
        proof_line(&response, &input, "response-to-input"),
    ];
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 32,
            connection_byte_capacity: 262_144,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .unwrap();

    assert_eq!(plan.checked_form_id, authored.expanded.checked_form_id);
    assert_eq!(plan.fragments.len(), 2);
    assert!(plan.fragments.iter().any(|fragment| fragment
        .connections
        .iter()
        .any(|connection| connection.selected_line.is_some())));
}
