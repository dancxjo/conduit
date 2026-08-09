use conduit_core::{
    selected_bound_link, BoundLink, ClueId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId,
    ControlLoopEvent, ControlLoopEventError, CredentialReferenceId, HostId, LinkAuthorityReference,
    LinkBindingId, LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
    PlannedConnection, PortId,
};

fn route(binding_id: &str) -> BoundLink {
    BoundLink {
        binding_id: LinkBindingId::from(binding_id),
        source: LinkEndpoint {
            host_id: HostId::from("source"),
            boot_id: conduit_core::BootId::from("source-boot"),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}-source")),
        },
        sink: LinkEndpoint {
            host_id: HostId::from("sink"),
            boot_id: conduit_core::BootId::from("sink-boot"),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}-sink")),
        },
        base: ConnectionBase::WebSocket,
        base_instance_id: ConnectionBaseInstanceId::from(format!("{binding_id}-base")),
        credential: LinkCredentialReference::Opaque(CredentialReferenceId::from("credential-ref")),
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 64,
            maximum_buffered_bytes: 64,
            maximum_frame_bytes: 128,
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
        base: ConnectionBase::WebSocket,
        link_binding: None,
        route_candidates: vec![route("route-a"), route("route-b")],
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
            request_clue_id: ClueId::from("planning-request"),
        },
        ControlLoopEvent::PlanningSucceeded {
            prior_plan_id: prior.clone(),
            replacement_plan_id: replacement.clone(),
            request_clue_id: ClueId::from("planning-request"),
            clue_id: ClueId::from("planning-succeeded"),
        },
        ControlLoopEvent::PlanSuperseded {
            prior_plan_id: prior,
            replacement_plan_id: replacement.clone(),
            clue_id: ClueId::from("plan-superseded"),
        },
        ControlLoopEvent::PlanRealized {
            plan_id: replacement,
            clue_id: ClueId::from("plan-realized"),
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
        request_clue_id: ClueId::from("request"),
        clue_id: ClueId::from("invalid-success"),
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
    let selected = ControlLoopEvent::RouteSelectionChanged {
        plan_id: plan_id.clone(),
        connection_id: connection.connection_id.clone(),
        previous_binding_id: Some(LinkBindingId::from("route-a")),
        selected_binding_id: LinkBindingId::from("route-b"),
        observation_clue_id: ClueId::from("route-b-ready"),
    };
    assert_eq!(selected.validate_route_event(&plan_id, &connection), Ok(()));
    assert_eq!(
        selected_bound_link(&selected, &connection).map(|candidate| candidate.binding_id.as_str()),
        Some("route-b")
    );

    let mut wrong_plan = selected.clone();
    if let ControlLoopEvent::RouteSelectionChanged { plan_id, .. } = &mut wrong_plan {
        *plan_id = PlanId::from("plan-b");
    }
    assert_eq!(
        wrong_plan.validate_route_event(&PlanId::from("plan-a"), &connection),
        Err(ControlLoopEventError::RouteEventPlanMismatch)
    );

    let outside = ControlLoopEvent::RouteSelectionChanged {
        plan_id,
        connection_id: connection.connection_id.clone(),
        previous_binding_id: Some(LinkBindingId::from("route-a")),
        selected_binding_id: LinkBindingId::from("route-c"),
        observation_clue_id: ClueId::from("route-c-ready"),
    };
    assert_eq!(
        outside.validate_route_event(&PlanId::from("plan-a"), &connection),
        Err(ControlLoopEventError::RouteOutsideSealedCandidates)
    );
}

#[test]
fn a_non_change_and_empty_clue_fail_closed() {
    let unchanged = ControlLoopEvent::RouteSelectionChanged {
        plan_id: PlanId::from("plan-a"),
        connection_id: ConnectionId::from("connection"),
        previous_binding_id: Some(LinkBindingId::from("route-a")),
        selected_binding_id: LinkBindingId::from("route-a"),
        observation_clue_id: ClueId::from("observation"),
    };
    assert_eq!(
        unchanged.validate(),
        Err(ControlLoopEventError::RouteSelectionDidNotChange)
    );
    let empty = ControlLoopEvent::PlanningRefused {
        prior_plan_id: PlanId::from("plan-a"),
        request_clue_id: ClueId::from("request"),
        reason: conduit_core::PlanningRefusalReason::NoCompatibleRealization,
        clue_id: ClueId::from(""),
    };
    assert_eq!(empty.validate(), Err(ControlLoopEventError::EmptyIdentity));
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
        "RouteSelectionChanged",
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
