use conduit_core::{
    selected_admitted_line, AdmittedLine, BaseImplementationId, BaseInstanceId, BoundLink,
    ConnectionId, ControlLoopEvent, ControlLoopEventError, CredentialReferenceId, HostId,
    LineContinuation, LineContract, LineDuplex, LineId, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape, LinkAuthorityReference, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, PlanId, PlannedConnection, PortId, SignId,
};

fn line(id: &str) -> AdmittedLine {
    AdmittedLine {
        line_id: LineId::from(id),
        binding: BoundLink {
            binding_id: LinkBindingId::from(id),
            source: LinkEndpoint {
                host_id: HostId::from("source"),
                boot_id: conduit_core::BootId::from("source-boot"),
                endpoint_id: LinkEndpointId::from(format!("{id}-source")),
            },
            sink: LinkEndpoint {
                host_id: HostId::from("sink"),
                boot_id: conduit_core::BootId::from("sink-boot"),
                endpoint_id: LinkEndpointId::from(format!("{id}-sink")),
            },
            base: BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            base_instance_id: BaseInstanceId::from(format!("{id}-base")),
            credential: LinkCredentialReference::Opaque(CredentialReferenceId::from(
                "credential-ref",
            )),
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 128,
            },
        },
        contract: LineContract {
            scope: LineScope::Machine,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
    }
}

fn connection() -> PlannedConnection {
    PlannedConnection {
        connection_id: ConnectionId::from("connection"),
        source_placement_id: conduit_core::PlacementId::from("source"),
        source_port_id: PortId::from("out"),
        sink_placement_id: conduit_core::PlacementId::from("sink"),
        sink_port_id: PortId::from("in"),
        value_kind: conduit_core::KindId::from("value/test"),
        temporal: conduit_core::PortTemporal::Value,
        selected_line: Some(line("line-a")),
        admitted_lines: vec![line("line-a"), line("line-b")],
        item_capacity: 1,
        byte_capacity: 64,
    }
}

#[test]
fn replacement_planning_and_same_plan_route_selection_are_distinct_records() {
    let prior = PlanId::from("plan-a");
    let replacement = PlanId::from("plan-b");
    let events = [
        ControlLoopEvent::PlanningRequested {
            prior_plan_id: prior.clone(),
            requester_host_id: HostId::from("planner-host"),
            requester_boot_id: conduit_core::BootId::from("planner-boot"),
            authority: conduit_core::PlanningRequestAuthority::HostLocal,
            request_sign_id: SignId::from("planning-request"),
        },
        ControlLoopEvent::PlanningSucceeded {
            prior_plan_id: prior.clone(),
            replacement_plan_id: replacement.clone(),
            request_sign_id: SignId::from("planning-request"),
            sign_id: SignId::from("planning-succeeded"),
        },
        ControlLoopEvent::PlanSuperseded {
            prior_plan_id: prior,
            replacement_plan_id: replacement.clone(),
            sign_id: SignId::from("plan-superseded"),
        },
        ControlLoopEvent::PlanRealized {
            plan_id: replacement,
            sign_id: SignId::from("plan-realized"),
        },
    ];
    assert!(events.iter().all(|event| event.validate().is_ok()));
    for (index, event) in events.iter().enumerate() {
        assert!(events[..index]
            .iter()
            .all(|prior| core::mem::discriminant(prior) != core::mem::discriminant(event)));
    }
}

#[test]
fn replacement_must_have_a_new_plan_identity() {
    let event = ControlLoopEvent::PlanningSucceeded {
        prior_plan_id: PlanId::from("same-plan"),
        replacement_plan_id: PlanId::from("same-plan"),
        request_sign_id: SignId::from("request"),
        sign_id: SignId::from("invalid-success"),
    };
    assert_eq!(
        event.validate(),
        Err(ControlLoopEventError::ReplacementReusedPlanIdentity)
    );
}

#[test]
fn route_selection_may_change_only_inside_the_same_sealed_plan() {
    let plan_id = PlanId::from("plan-a");
    let connection = connection();
    let selected = ControlLoopEvent::LineSelectionChanged {
        plan_id: plan_id.clone(),
        connection_id: connection.connection_id.clone(),
        previous_line_id: Some(LineId::from("line-a")),
        selected_line_id: LineId::from("line-b"),
        selected_binding_id: LinkBindingId::from("line-b"),
        observation_sign_id: SignId::from("route-b-ready"),
    };
    assert_eq!(selected.validate_route_event(&plan_id, &connection), Ok(()));
    assert_eq!(
        selected_admitted_line(&selected, &connection)
            .map(|candidate| candidate.binding.binding_id.as_str()),
        Some("line-b")
    );

    let mut wrong_plan = selected.clone();
    if let ControlLoopEvent::LineSelectionChanged { plan_id, .. } = &mut wrong_plan {
        *plan_id = PlanId::from("plan-b");
    }
    assert_eq!(
        wrong_plan.validate_route_event(&PlanId::from("plan-a"), &connection),
        Err(ControlLoopEventError::RouteEventPlanMismatch)
    );

    let outside = ControlLoopEvent::LineSelectionChanged {
        plan_id,
        connection_id: connection.connection_id.clone(),
        previous_line_id: Some(LineId::from("line-a")),
        selected_line_id: LineId::from("line-c"),
        selected_binding_id: LinkBindingId::from("line-c"),
        observation_sign_id: SignId::from("route-c-ready"),
    };
    assert_eq!(
        outside.validate_route_event(&PlanId::from("plan-a"), &connection),
        Err(ControlLoopEventError::RouteOutsideSealedCandidates)
    );
}

#[test]
fn a_non_change_and_empty_sign_fail_closed() {
    let unchanged = ControlLoopEvent::LineSelectionChanged {
        plan_id: PlanId::from("plan-a"),
        connection_id: ConnectionId::from("connection"),
        previous_line_id: Some(LineId::from("line-a")),
        selected_line_id: LineId::from("line-a"),
        selected_binding_id: LinkBindingId::from("line-a"),
        observation_sign_id: SignId::from("observation"),
    };
    assert_eq!(
        unchanged.validate(),
        Err(ControlLoopEventError::RouteSelectionDidNotChange)
    );
    let empty = ControlLoopEvent::PlanningRefused {
        prior_plan_id: PlanId::from("plan-a"),
        request_sign_id: SignId::from("request"),
        reason: conduit_core::PlanningRefusalReason::NoCompatibleRealization,
        sign_id: SignId::from(""),
    };
    assert_eq!(empty.validate(), Err(ControlLoopEventError::EmptyIdentity));
}

#[test]
fn unavailable_host_must_be_exactly_sealed_by_the_same_plan() {
    let plan = conduit_core::seal_plan(
        conduit_core::FormIdentity {
            source_document_id: conduit_core::SourceDocumentId::from("source"),
            checked_form_id: conduit_core::CheckedFormId::from("checked"),
            expanded_form_id: conduit_core::ExpandedFormId::from("expanded"),
        },
        vec![conduit_core::PlanFragment {
            plan_id: PlanId::from(""),
            fragment_id: conduit_core::FragmentId::from(""),
            source_document_id: conduit_core::SourceDocumentId::from("source"),
            checked_form_id: conduit_core::CheckedFormId::from("checked"),
            expanded_form_id: conduit_core::ExpandedFormId::from("expanded"),
            realization_backs: vec![],
            host_id: HostId::from("host-a"),
            boot_id: conduit_core::BootId::from("boot-a"),
            offer_generation: conduit_core::OfferGeneration(4),
            placements: vec![],
            execution_regions: vec![],
            execution_fusions: vec![],
            states: Vec::new(),
            connections: vec![],
            shared_pools: vec![],
            startup_dependencies: vec![],
            startup_order: vec![],
            cancellation_policy: conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![],
            expected_sign: vec![],
            sign_storage_budget: conduit_core::SignStorageBudget {
                item_capacity: 0,
                byte_capacity: 0,
            },
            plan_fragments: vec![],
        }],
    );
    let exact = ControlLoopEvent::HostBecameUnavailable {
        plan_id: plan.plan_id.clone(),
        host_id: HostId::from("host-a"),
        boot_id: conduit_core::BootId::from("boot-a"),
        offer_generation: conduit_core::OfferGeneration(4),
        observation_sign_id: SignId::from("host-a-lost"),
    };
    assert_eq!(exact.validate_host_event(&plan), Ok(()));

    let mut malformed_plan = plan.clone();
    malformed_plan.plan_id = PlanId::from("tampered");
    assert_eq!(
        exact.validate_host_event(&malformed_plan),
        Err(ControlLoopEventError::InvalidPlan)
    );

    let mut stale_boot = exact.clone();
    if let ControlLoopEvent::HostBecameUnavailable { boot_id, .. } = &mut stale_boot {
        *boot_id = conduit_core::BootId::from("boot-stale");
    }
    assert_eq!(
        stale_boot.validate_host_event(&plan),
        Err(ControlLoopEventError::HostOutsideSealedPlan)
    );

    let mut wrong_plan = exact;
    if let ControlLoopEvent::HostBecameUnavailable { plan_id, .. } = &mut wrong_plan {
        *plan_id = PlanId::from("other-plan");
    }
    assert_eq!(
        wrong_plan.validate_host_event(&plan),
        Err(ControlLoopEventError::HostEventPlanMismatch)
    );
}

#[test]
fn architecture_contract_keeps_replan_and_route_change_language_distinct() {
    let document =
        include_str!("../../../docs/architecture/topology-planning-play-control-loop.md");
    for required in [
        "A Plan is immutable",
        "PlanningRequested",
        "PlanningRefused",
        "PlanningSucceeded",
        "PlanSuperseded",
        "PlanRealized",
        "LineSelectionChanged",
        "#466",
        "#495",
        "#496",
    ] {
        assert!(document.contains(required), "missing {required}");
    }
    assert!(document.contains("not an automatic command to plan"));
    assert!(document.contains("unchanged `PlanId`"));
    assert!(document.contains("no host is a privileged coordinator"));
}
