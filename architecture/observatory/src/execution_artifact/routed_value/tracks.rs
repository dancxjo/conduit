//! Typed Source/Plan envelope fixtures, not a claimed kernel execution.
use super::*;
use crate::PlanLifecycle;
use alloc::{collections::BTreeMap, format, string::ToString, vec};
use conduit_core::*;
use conduit_plot::*;

#[path = "tracks_kernel.rs"]
mod kernel_tests;

fn kind(name: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> Kind {
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: format!("{name}@1").into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs,
        outputs,
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 4,
            max_queue_bytes: 1024,
        },
    }
}
fn artifact(track: ConnectionTrack) -> RoutedValueArtifact {
    artifact_context(track, false)
}
fn artifact_context(track: ConnectionTrack, fore: bool) -> RoutedValueArtifact {
    let carried = match track {
        ConnectionTrack::Payload => TEXT_INFO_ID,
        ConnectionTrack::AbnormalTerminal => BYTES_INFO_ID,
        ConnectionTrack::NormalClose | ConnectionTrack::Quiescence => EMPTY_INFO_ID,
    };
    let source = kind(
        "fixture/source",
        vec![],
        vec![PortDescriptor {
            port_id: port_id("out"),
            value_kind: kind_id(TEXT_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: if fore && track == ConnectionTrack::Payload {
                // The authored payload Fore declares a plain closing Text stream.
                None
            } else {
                Some(kind_id(BYTES_INFO_ID))
            },
        }],
    );
    let sink = kind(
        "fixture/sink",
        vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: kind_id(carried),
            direction: PortDirection::Input,
            temporal: if track == ConnectionTrack::Payload {
                PortTemporal::Flow { closes: true }
            } else {
                PortTemporal::Value
            },
            abnormal_kind: None,
        }],
        vec![],
    );
    let mut profile = ProfileCatalog::new();
    for k in [&source, &sink] {
        profile.insert_kind(k.clone()).unwrap();
    }
    let suffix = match track {
        ConnectionTrack::Payload => "",
        ConnectionTrack::NormalClose => "|",
        ConnectionTrack::AbnormalTerminal => "!",
        ConnectionTrack::Quiescence => ";",
    };
    let text = if fore {
        let ty = match track {
            ConnectionTrack::Payload => "Text...|",
            ConnectionTrack::AbnormalTerminal => "Bytes",
            _ => "Empty",
        };
        format!("plot main (\n result: {ty} >>\n) {{\n sender: fixture/source\n sender.out{suffix} >> result\n}}\n")
    } else {
        format!("plot main {{\n sender: fixture/source\n receiver: fixture/sink\n sender.out{suffix} >> receiver.input\n}}\n")
    };
    let checked = check_syntax_document(
        &parse_syntax_document(&text),
        &profile.startup_catalog().unwrap(),
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "main", &profile).unwrap();
    let offers = [source, sink]
        .into_iter()
        .map(|k| {
            let name = k.kind_id.as_str().to_string();
            BackOfferBuilder::new(
                k,
                Back {
                    capability_id: name.clone().into(),
                    execution_profile_id: format!("{name}/profile@1").into(),
                    implementation_id: format!("{name}/implementation").into(),
                    artifact_id: format!("{name}/artifact").into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture-host".into(),
        boot_id: "fixture-boot".into(),
        profile: "fixture/profile@1".into(),
        offer_generation: OfferGeneration(1),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let plan = if fore {
        let boundaries = BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("result"),
                track,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 1024,
            },
        )]);
        conduit_planner::plan_expanded_authoring_with_options(
            &expanded,
            &hosts,
            &choices,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 1024,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap()
    } else {
        conduit_planner::plan_expanded_canonical(
            &expanded.expanded,
            &hosts,
            &choices,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )
        .unwrap()
    };
    assert!(verify_plan(&plan), "track={track:?} fore={fore}");
    let (placement, output_port, connection_id) = if fore {
        assert!(plan.fragments[0].connections.is_empty());
        let port = &plan.fragments[0].fore_ports[0];
        assert_eq!(port.track, track);
        assert_eq!(port.value_kind.as_str(), carried);
        (port.placement_id.clone(), port.gear_port_id.clone(), None)
    } else {
        let connection = &plan.fragments[0].connections[0];
        assert_eq!(connection.track, track);
        assert_eq!(connection.value_kind.as_str(), carried);
        (
            connection.source_placement_id.clone(),
            connection.source_port_id.clone(),
            Some(connection.connection_id.clone()),
        )
    };
    let active = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        1,
    );
    let identity = bind_sign(
        &active.host_id,
        &active.boot_id,
        Some(&active.active_play_id),
        1,
    );
    let bytes = if matches!(
        track,
        ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
    ) {
        vec![]
    } else {
        b"sample".to_vec()
    };
    let play = PlayArtifact {
        schema: PLAY_ARTIFACT_SCHEMA.into(),
        identity: active.clone(),
        plan: plan.clone(),
        play: crate::PlayReport {
            active_play_id: active.active_play_id.clone(),
            plan_id: plan.plan_id.clone(),
            host_id: active.host_id.clone(),
            boot_id: active.boot_id.clone(),
            lifecycle: PlanLifecycle::Active,
            terminal_disposition: None,
            failure_message: None,
            placements: vec![],
            connections: vec![],
        },
    };
    let sign = SignArtifact {
        schema: SIGN_ARTIFACT_SCHEMA.into(),
        identity: identity.clone(),
        plan: None,
        play: Some(play),
        sign: Observation {
            sign_id: identity.sign_id,
            active_play_id: Some(active.active_play_id),
            face_id: None,
            host_id: active.host_id,
            boot_id: active.boot_id,
            plan_id: Some(plan.plan_id),
            placement_id: Some(placement),
            connection_id,
            kind: {
                let evidence = RoutedContentEvidence {
                    native_event_sequence: 1,
                    output_port,
                    byte_length: bytes.len() as u32,
                    digest_domain: "conduit/routed-value-content@1".into(),
                    digest_scheme: "core/semantic-sha256@1".into(),
                    digest: semantic_digest("conduit/routed-value-content@1", &bytes),
                };
                if fore {
                    ObservationKind::ForeOutputContent {
                        front_port_id: port_id("result"),
                        track,
                        evidence,
                    }
                } else {
                    ObservationKind::RoutedContent { evidence }
                }
            },
        },
    };
    RoutedValueArtifact::new(sign, bytes).unwrap()
}
#[test]
fn each_track_artifact_names_its_carried_kind_and_rejects_payload_relabelling() {
    for (track, expected) in [
        (ConnectionTrack::Payload, TEXT_INFO_ID),
        (ConnectionTrack::NormalClose, EMPTY_INFO_ID),
        (ConnectionTrack::AbnormalTerminal, BYTES_INFO_ID),
        (ConnectionTrack::Quiescence, EMPTY_INFO_ID),
    ] {
        let value = artifact(track);
        assert_eq!(value.value_kind.as_str(), expected);
        validate_routed_value_artifact(&value).unwrap();
        let roundtrip: RoutedValueArtifact =
            serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(roundtrip, value);
        if track != ConnectionTrack::Payload {
            let mut wrong = value;
            wrong.value_kind = kind_id(TEXT_INFO_ID);
            assert!(validate_routed_value_artifact(&wrong).is_err());
        }
    }
}

#[test]
fn fore_output_artifacts_cover_all_tracks_without_inventing_a_cord() {
    for track in [
        ConnectionTrack::Payload,
        ConnectionTrack::NormalClose,
        ConnectionTrack::AbnormalTerminal,
        ConnectionTrack::Quiescence,
    ] {
        let value = artifact_context(track, true);
        assert!(value.sign.sign.connection_id.is_none());
        validate_routed_value_artifact(&value).unwrap();
        let roundtrip: RoutedValueArtifact =
            serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(value, roundtrip);
        for mutation in 0..5 {
            let mut wrong = value.clone();
            match mutation {
                0 => wrong.sign.sign.connection_id = Some("invented-cord".into()),
                1 => {
                    if let ObservationKind::ForeOutputContent { front_port_id, .. } =
                        &mut wrong.sign.sign.kind
                    {
                        *front_port_id = port_id("unsealed");
                    }
                }
                2 => {
                    if let ObservationKind::ForeOutputContent { track, .. } =
                        &mut wrong.sign.sign.kind
                    {
                        *track = if *track == ConnectionTrack::Payload {
                            ConnectionTrack::NormalClose
                        } else {
                            ConnectionTrack::Payload
                        };
                    }
                }
                3 => {
                    if let ObservationKind::ForeOutputContent { evidence, .. } =
                        &mut wrong.sign.sign.kind
                    {
                        evidence.output_port = port_id("wrong-port");
                    }
                }
                _ => {
                    if let ObservationKind::ForeOutputContent { evidence, .. } =
                        &wrong.sign.sign.kind
                    {
                        wrong.sign.sign.kind = ObservationKind::RoutedContent {
                            evidence: evidence.clone(),
                        };
                    }
                }
            }
            assert!(
                validate_routed_value_artifact(&wrong).is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn sealed_quiescence_fore_keeps_empty_kind_and_requires_a_flow_producer() {
    let value = artifact_context(ConnectionTrack::Quiescence, true);
    let plan = &value.sign.play.as_ref().unwrap().plan;
    let reseal = |fragments| {
        seal_plan_with_completion(
            PlotIdentity {
                source_document_id: plan.source_document_id.clone(),
                checked_plot_id: plan.checked_plot_id.clone(),
                expanded_plot_id: plan.expanded_plot_id.clone(),
            },
            plan.completion_policy,
            fragments,
        )
    };
    assert!(verify_plan(&reseal(plan.fragments.clone())));
    for mutation in 0..3 {
        let mut fragments = plan.fragments.clone();
        match mutation {
            0 => fragments[0].fore_ports[0].value_kind = kind_id(TEXT_INFO_ID),
            1 => fragments[0].fore_ports[0].temporal = PortTemporal::Flow { closes: true },
            _ => fragments[0].placements[0].outputs[0].temporal = PortTemporal::Value,
        }
        // Resealing keeps this a semantic rejection rather than a stale hash.
        assert!(!verify_plan(&reseal(fragments)), "mutation {mutation}");
    }
}

#[test]
fn empty_control_tracks_refuse_nonempty_bytes_despite_a_matching_digest() {
    for fore in [false, true] {
        for track in [ConnectionTrack::NormalClose, ConnectionTrack::Quiescence] {
            let mut value = artifact_context(track, fore);
            value.bytes.push(42);
            let evidence = match &mut value.sign.sign.kind {
                ObservationKind::RoutedContent { evidence }
                | ObservationKind::ForeOutputContent { evidence, .. } => evidence,
                _ => unreachable!(),
            };
            evidence.byte_length = 1;
            evidence.digest = semantic_digest(&evidence.digest_domain, &value.bytes);
            assert!(
                validate_routed_value_artifact(&value).is_err(),
                "track={track:?} fore={fore}"
            );
        }
    }
}
