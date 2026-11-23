//! Minimum typed two-Host Plan for the product's selected pool wire tests.
//! Mirrors the active std session fixture without exporting test-only std APIs.
use conduit_core::*;
use conduit_std_host::pool_member_sessions::PoolMemberSessions;

pub(super) fn sessions() -> PoolMemberSessions {
    let (plan, selection, consumer) = exact_plan();
    assert!(verify_plan(&plan));
    PoolMemberSessions::prepare(&plan, selection, &consumer, &HostId::from("host/worker")).unwrap()
}

fn endpoint(host: &str, boot: &str, endpoint: &str) -> LinkEndpoint {
    LinkEndpoint {
        host_id: HostId::from(host),
        boot_id: conduit_core::BootId::from(boot),
        endpoint_id: LinkEndpointId::from(endpoint),
    }
}

fn line(id: &str, source: LinkEndpoint, sink: LinkEndpoint) -> AdmittedLine {
    AdmittedLine {
        line_id: LineId::from(id),
        binding: BoundLink {
            binding_id: LinkBindingId::from(format!("{id}/binding")),
            source,
            sink,
            base: BaseImplementationId::from("conduit.base/test-session@1"),
            base_instance_id: BaseInstanceId::from(format!("{id}/instance")),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 4_096,
            },
        },
        contract: LineContract {
            scope: LineScope::LocalNetwork,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::AuthenticatedEncrypted,
        },
    }
}

fn fragment(
    host: &str,
    boot: &str,
    placements: Vec<PlannedGear>,
    pool: PlannedSharedPool,
) -> PlanFragment {
    let expected_sign = if placements.is_empty() {
        vec![
            ExpectedSign::PlanFragmentReceived,
            ExpectedSign::PlanTerminal,
        ]
    } else {
        vec![
            ExpectedSign::PlanFragmentReceived,
            ExpectedSign::PlacementPrepared(placements[0].placement_id.clone()),
            ExpectedSign::PlacementTerminal(placements[0].placement_id.clone()),
            ExpectedSign::PlanTerminal,
        ]
    };
    let expected_terminals = placements
        .iter()
        .map(|placement| ExpectedTerminal::PlacementCompleted(placement.placement_id.clone()))
        .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
        .collect();
    PlanFragment {
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source/pool-session"),
        checked_plot_id: conduit_core::CheckedPlotId::from("checked/pool-session"),
        expanded_plot_id: ExpandedPlotId::from("expanded/pool-session"),
        completion_policy: conduit_core::PlanCompletionPolicy::Live,
        realization_backs: vec![],
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        startup_order: placements
            .iter()
            .map(|placement| placement.placement_id.clone())
            .collect(),
        placements,
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![pool],
        startup_dependencies: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals,
        sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap(),
        expected_sign,
        plan_fragments: vec![],
    }
}

fn exact_plan() -> (Plan, PoolSelectionEvidence, PlacementId) {
    let consumer_id = PlacementId::from("placement/client");
    let input = PortDescriptor {
        port_id: PortId::from("prompt"),
        value_kind: KindId::from("value/text"),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: PortId::from("text"),
        value_kind: KindId::from("value/text"),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let pool_id = SharedPoolId::from("pool/workers");
    let request = line(
        "line/request",
        endpoint("host/client", "boot/client", "client/request"),
        endpoint("host/worker", "boot/worker", "worker/request"),
    );
    let result = line(
        "line/result",
        endpoint("host/worker", "boot/worker", "worker/result"),
        endpoint("host/client", "boot/client", "client/result"),
    );
    let pool = PlannedSharedPool {
        pool_id: pool_id.clone(),
        declaration_id: PoolDeclarationId::from("declaration/workers"),
        member_front: CheckedFront::new(vec![], vec![input], vec![output], None),
        maximum_members: 1,
        member_limits: PoolMemberLimits {
            queue_item_capacity: 1,
            queue_byte_capacity: 64,
            sign_item_capacity: 8,
            sign_byte_capacity: 1_024,
        },
        member_sessions_required: true,
        realization_envelope: vec![PoolRealizationEnvelope {
            host_id: HostId::from("host/worker"),
            boot_id: BootId::from("boot/worker"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("capability/worker"),
            implementation_id: ImplementationId::from("implementation/worker"),
            artifact_id: ArtifactId::from("artifact/worker"),
            member_capacity: 1,
            resources: vec![],
            admitted_lines: vec![request, result],
        }],
        selection_policy: SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        admission_authority: AuthorityGrantId::from("grant/workers"),
        consumers: vec![consumer_id.clone()],
    };
    let consumer = conduit_core::planned_gear_from_parts! {
        semantic_contract: Default::default(),
        placement_id: consumer_id.clone(),
        gear_id: GearId::from("client"),
        kind_id: KindId::from("flow/pool-observe"),
        kind_contract_revision: KindIdentity::from("flow/pool-observe@1"),
        execution_profile_id: ExecutionProfileId::from("test@1"),
        configuration: Vec::<ConfigurationEntry>::new(),
        host_id: HostId::from("host/client"),
        boot_id: BootId::from("boot/client"),
        offer_generation: OfferGeneration(1),
        capability_id: CapabilityId::from("capability/client"),
        implementation_id: ImplementationId::from("implementation/client"),
        artifact_id: ArtifactId::from("artifact/client"),
        base: None,
        realization_characteristics: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
        inputs: vec![],
        outputs: vec![],
        terminal_transductions: Vec::new(),
        host_calls: vec![],
        resources: vec![],
        authority: vec![],
        pool_references: vec![pool_id.clone()],
    };
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: SourceDocumentId::from("source/pool-session"),
            checked_plot_id: conduit_core::CheckedPlotId::from("checked/pool-session"),
            expanded_plot_id: ExpandedPlotId::from("expanded/pool-session"),
        },
        vec![
            fragment("host/client", "boot/client", vec![consumer], pool.clone()),
            fragment("host/worker", "boot/worker", vec![], pool),
        ],
    );
    let selection = PoolSelectionEvidence {
        plan_id: plan.plan_id.clone(),
        pool_id,
        operation_id: PoolOperationId::from("request/1"),
        selected_realization: Some(0),
        observation_sign_ids: vec![SignId::from("sign/provider/1")],
        disposition: PoolSelectionDisposition::Selected,
        sign_id: SignId::from("sign/selection/1"),
    };
    (plan, selection, consumer_id)
}
