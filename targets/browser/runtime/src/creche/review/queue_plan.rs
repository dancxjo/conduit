//! Review uses ordinary finite Cord budgets supported by both selected offers.
use conduit_core::{
    authority_grant, process_owned_line_offer_with_limits, AuthorityGrant, BaseImplementationId,
    HostAdvertisement, LineId, LineOffer, LineScope, LineSecurity, LinkLimits,
};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::{ConnectionQueueLimits, PlanningOptions};
use std::collections::BTreeMap;

pub(in crate::creche) fn plan(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    bases: &[BaseImplementationId],
    joined_lines: &[crate::creche::JoinedLineObservation],
    authority: crate::creche::PlanningAuthority,
) -> Result<conduit_core::Plan, String> {
    let mut limits = BTreeMap::new();
    for cord in &form.connections {
        let capability = |gear| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or("missing review placement")?;
            hosts
                .iter()
                .find(|host| host.host_id == choice.host_id)
                .and_then(|host| {
                    host.capabilities
                        .iter()
                        .find(|offer| offer.capability_id == choice.capability_id)
                })
                .ok_or("missing selected review capability")
        };
        let source = capability(&cord.source_gear_id)?;
        let sink = capability(&cord.sink_gear_id)?;
        limits.insert(
            (
                cord.source_gear_id.clone(),
                cord.source_port_id.clone(),
                cord.sink_gear_id.clone(),
                cord.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: source
                    .limits
                    .max_queue_items
                    .min(sink.limits.max_queue_items)
                    .min(4),
                byte_capacity: source
                    .limits
                    .max_queue_bytes
                    .min(sink.limits.max_queue_bytes),
            },
        );
    }
    let authority_grants = authority_grants(hosts, placements, authority)?;
    let (line_offers, line_candidates) =
        line_offers(form, hosts, placements, &limits, joined_lines)?;
    let mut available_bases = bases.to_vec();
    if !line_offers.is_empty()
        && !available_bases
            .iter()
            .any(|base| base.as_str() == "conduit.base/websocket-rfc6455@1")
    {
        available_bases.push(BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        ));
    }
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        form,
        hosts,
        placements,
        &available_bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &line_offers,
        },
        &limits,
    )
    .map_err(|error| error.to_string())
}

fn authority_grants(
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    authority: crate::creche::PlanningAuthority,
) -> Result<Vec<AuthorityGrant>, String> {
    let mut grants = Vec::new();
    for (gear, placement) in &placements.by_gear {
        let host = hosts
            .iter()
            .find(|host| host.host_id == placement.host_id)
            .ok_or("Workspace placement names no current host")?;
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or("Workspace placement names no current capability")?;
        for (index, requirement) in capability.authority_requirements.iter().enumerate() {
            let browser_audio = matches!(
                requirement.contract_id.as_str(),
                "conduit.authority/request-browser-microphone@1"
                    | "conduit.authority/use-browser-audio-output@1"
            );
            if !browser_audio || !authority.browser_audio {
                continue;
            }
            grants.push(authority_grant(
                &format!("grant/workspace/{}/{index}", gear.as_str()),
                requirement,
                host.host_id.clone(),
                host.boot_id.clone(),
                capability.capability_id.clone(),
            ));
        }
    }
    Ok(grants)
}

type LineCandidates = BTreeMap<(conduit_core::GearId, conduit_core::GearId), Vec<LineId>>;

fn line_offers(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    limits: &BTreeMap<
        (
            conduit_core::GearId,
            conduit_core::PortId,
            conduit_core::GearId,
            conduit_core::PortId,
        ),
        ConnectionQueueLimits,
    >,
    joined_lines: &[crate::creche::JoinedLineObservation],
) -> Result<(Vec<LineOffer>, LineCandidates), String> {
    let mut offers = Vec::new();
    let mut candidates = BTreeMap::new();
    for (index, connection) in form.connections.iter().enumerate() {
        let source_placement = placements
            .by_gear
            .get(&connection.source_gear_id)
            .ok_or("Workspace Cord has no source placement")?;
        let sink_placement = placements
            .by_gear
            .get(&connection.sink_gear_id)
            .ok_or("Workspace Cord has no sink placement")?;
        if source_placement.host_id == sink_placement.host_id {
            continue;
        }
        let source = hosts
            .iter()
            .find(|host| host.host_id == source_placement.host_id)
            .ok_or("Workspace Cord source Host is absent")?;
        let sink = hosts
            .iter()
            .find(|host| host.host_id == sink_placement.host_id)
            .ok_or("Workspace Cord sink Host is absent")?;
        let joined = joined_lines.iter().find(|line| {
            line.carrier == "conduit-line/loopback-websocket@1"
                && ((line.host_id == source.host_id && line.boot_id == source.boot_id)
                    || (line.host_id == sink.host_id && line.boot_id == sink.boot_id))
        });
        if joined.is_none() {
            continue;
        }
        let cord_limits = limits
            .get(&(
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ))
            .ok_or("Workspace Cord limits are absent")?;
        let maximum_frame_bytes = cord_limits
            .byte_capacity
            .checked_add(8_192)
            .ok_or("Workspace Line frame capacity overflow")?;
        let line_id = format!("line/workspace/{index}");
        let mut offer = process_owned_line_offer_with_limits(
            &line_id,
            &format!("binding/workspace/{index}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base/workspace/joined-websocket",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: cord_limits.item_capacity,
                maximum_payload_bytes: cord_limits.byte_capacity,
                maximum_buffered_bytes: cord_limits.byte_capacity,
                maximum_frame_bytes,
            },
        );
        offer.contract.scope = LineScope::LocalNetwork;
        offer.contract.security = LineSecurity::PlaintextNetwork;
        candidates.insert(
            (
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            ),
            vec![offer.line_id.clone()],
        );
        offers.push(offer);
    }
    Ok((offers, candidates))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
        ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
        OfferGeneration, PROTOCOL_VERSION,
    };
    use conduit_form::KindDefinition;

    fn remote_offer(definition: &KindDefinition) -> CapabilityOffer {
        let slug = definition.kind_id.as_str().replace('/', "-");
        CapabilityOffer {
            startup_parameters: definition
                .configuration
                .iter()
                .map(|field| conduit_core::FrontStartupParameter {
                    name: field.key.clone(),
                    value_type: conduit_core::kind_id(match field.default_value {
                        ConfigurationValue::Bool(_) => "value/bool",
                        ConfigurationValue::I64(_) => "value/scalar",
                        ConfigurationValue::U64(_) => "value/count",
                        ConfigurationValue::Text(_) => "value/text",
                        ConfigurationValue::Structured(ref value) => value.profile().as_str(),
                    }),
                    has_default: true,
                })
                .collect(),
            shorthand: None,
            capability_id: CapabilityId::from(format!("voice-host/{slug}")),
            kind_id: definition.kind_id.clone(),
            kind_contract_revision: definition.kind_contract_revision.clone(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("voice-host/profile@1"),
                implementation_id: ImplementationId::from(format!("voice-host/{slug}@1")),
                artifact_id: ArtifactId::from(format!("voice-host/{slug}-artifact@1")),
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

    #[test]
    fn spoken_conversation_requires_explicit_audio_authority_and_a_joined_line() {
        let source = include_str!("../../../../../../forms/live-conversation/main.conduit");
        let (startup, mut profile) = crate::installed_browser::catalogs_for_presentation(
            crate::installed_browser::PresentationProfile::Annotation,
        )
        .unwrap();
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let selector_offers =
            crate::installed_browser::catalogs::install_checked_structured_selectors(
                &checked,
                &mut profile,
            )
            .unwrap();
        let backs = crate::installed_browser::backs(&startup, &profile).unwrap();
        let expanded = conduit_form::expand_canonical_form_with_backs(
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
                .map(|gear| remote_offer(profile.get(&gear.kind_id).unwrap()))
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
                work: conduit_ai::LlmWorkBounds {
                    maximum_input_bytes: 4_096,
                    maximum_context_items: 1,
                    maximum_output_bytes: 4_096,
                    maximum_work_units: 4_096,
                    maximum_history_items: 0,
                },
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
            conduit_std_offers::piper_streaming_speech_offer(),
        ];
        offers.extend(model.capability_offers().unwrap());
        offers
    }

    #[test]
    fn actual_std_voice_offers_cover_every_expanded_remote_conversation_gear() {
        let source = include_str!("../../../../../../forms/live-conversation/main.conduit");
        let (startup, mut profile) = crate::installed_browser::catalogs_for_presentation(
            crate::installed_browser::PresentationProfile::Annotation,
        )
        .unwrap();
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        crate::installed_browser::catalogs::install_checked_structured_selectors(
            &checked,
            &mut profile,
        )
        .unwrap();
        let backs = crate::installed_browser::backs(&startup, &profile).unwrap();
        let expanded = conduit_form::expand_canonical_form_with_backs(
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
}
