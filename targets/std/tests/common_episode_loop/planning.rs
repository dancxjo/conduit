//! Exact planning shared by the installed episode proofs.
use super::*;
pub fn plan_host(
    mut host: StdHost,
    program: &str,
    entry: &str,
    fore: &[(&str, PortDirection, u32)],
    experience_evidence: bool,
    install: impl FnOnce(&mut StartupCatalog, &mut ProfileCatalog) -> Result<(), String>,
) -> Harness {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile).unwrap();
    conduit_chat::install_episode_loop_catalog(&mut startup, &mut profile).unwrap();
    install(&mut startup, &mut profile).unwrap();
    if experience_evidence {
        conduit_semantic_catalog::install_experience_evidence_catalogs(&mut startup, &mut profile)
            .unwrap();
    }
    let syntax = parse_syntax_document(program);
    assert_eq!(syntax.round_trip(), program);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &profile).unwrap();
    for gear in &authoring.expanded.gears {
        if gear.kind_id.as_str() == conduit_core::STATE_VALUE_KIND {
            let conduit_core::ConfigurationValue::Structured(initial) = &gear
                .configuration
                .iter()
                .find(|entry| entry.key == "initial")
                .unwrap()
                .value
            else {
                panic!("native keep initializer")
            };
            let initial =
                conduit_core::StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
                    .unwrap();
            host.install_play_state_value(initial.value_type(), &initial)
                .unwrap();
        }
        if gear.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
            let conduit_core::ConfigurationValue::Text(encoded) = &gear
                .configuration
                .iter()
                .find(|c| c.key == "program")
                .unwrap()
                .value
            else {
                panic!("checked filter program")
            };
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            host.install_pure_filter(&program, gear.inputs[0].temporal)
                .unwrap();
        }
    }
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = fore
        .iter()
        .copied()
        .map(|(name, direction, byte_capacity)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: conduit_core::port_id(name),
                    track: ConnectionTrack::Payload,
                },
                // One queued item per Fore: later observations wait upstream.
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect();
    let offer = |gear: &conduit_core::GearId| {
        let choice = placements.by_gear.get(gear).unwrap();
        hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
            .unwrap()
            .capabilities
            .iter()
            .find(|cap| cap.capability_id == choice.capability_id)
            .unwrap()
    };
    let connection_limits = authoring
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let source = authoring
                .expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == cord.source_gear_id)
                .unwrap();
            let maximum =
                if source.kind_id.as_str() == conduit_chat::EPISODE_RETRIEVAL_PRODUCER_KIND {
                    match cord.source_port_id.as_str() {
                        "capture" => conduit_chat::MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES as u32,
                        "window" => conduit_ai::TEMPORAL_EVIDENCE_FLOW_MAX_BYTES as u32,
                        _ => 16384,
                    }
                } else {
                    16384
                };
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: maximum
                        .min(offer(&cord.source_gear_id).limits.max_queue_bytes)
                        .min(offer(&cord.sink_gear_id).limits.max_queue_bytes),
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16_384,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    let fragment = plan.fragments[0].clone();
    let expected: &[(&str, &str)] = if experience_evidence {
        &[
            (
                conduit_semantic_catalog::EXPERIENCE_MODEL_REQUEST_KIND,
                conduit_std_offers::EXPERIENCE_MODEL_REQUEST_IMPLEMENTATION,
            ),
            (
                conduit_ai::LLM_GENERATE_FLOW_KIND,
                conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
            ),
        ]
    } else {
        &[
            (
                conduit_chat::EPISODE_COMPOSE_KIND,
                conduit_std_offers::EPISODE_COMPOSE_IMPLEMENTATION,
            ),
            (
                conduit_chat::EPISODE_RETAIN_KIND,
                conduit_std_offers::EPISODE_RETAIN_IMPLEMENTATION,
            ),
            (
                conduit_chat::EPISODE_MEMORY_KIND,
                conduit_std_offers::EPISODE_MEMORY_IMPLEMENTATION,
            ),
            (
                conduit_ai::LLM_GENERATE_FLOW_KIND,
                conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
            ),
        ]
    };
    for &(mut kind, mut implementation) in expected {
        if matches!(
            entry,
            "experience-episode-loop" | "pete-cognition" | "pete-cognition-resumable"
        ) && kind == conduit_chat::EPISODE_COMPOSE_KIND
        {
            kind = conduit_chat::EPISODE_EXPERIENCE_COMPOSE_KIND;
            implementation = conduit_std_offers::EPISODE_EXPERIENCE_COMPOSE_IMPLEMENTATION;
            if entry.starts_with("pete-cognition") {
                kind = conduit_chat::EPISODE_RETRIEVAL_PRODUCER_KIND;
                implementation = conduit_std_offers::EPISODE_RETRIEVAL_PRODUCER_IMPLEMENTATION;
            }
        }
        if entry == "pete-cognition-resumable" && kind == conduit_chat::EPISODE_MEMORY_KIND {
            kind = conduit_chat::EPISODE_RESUMABLE_MEMORY_KIND;
            implementation = conduit_std_offers::EPISODE_RESUMABLE_MEMORY_IMPLEMENTATION;
        }
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(placement.implementation_id.as_str(), implementation);
    }
    if entry.starts_with("pete-cognition") {
        assert!(fragment
            .placements
            .iter()
            .any(|placement| placement.implementation_id.as_str()
                == conduit_std_offers::EPISODE_EXPERIENCE_INTEGRATION_IMPLEMENTATION));
        assert!(fragment
            .placements
            .iter()
            .any(|placement| placement.implementation_id.as_str()
                == conduit_std_offers::EPISODE_RETRIEVAL_CONTEXT_IMPLEMENTATION));
        assert!(fragment
            .placements
            .iter()
            .any(|placement| placement.implementation_id.as_str()
                == conduit_std_offers::TEMPORAL_EVIDENCE_FLOW_IMPLEMENTATION));
    }
    Harness {
        host,
        requests: Arc::default(),
        fragment,
        plan_id: plan.plan_id.as_str().into(),
        authoring,
    }
}
