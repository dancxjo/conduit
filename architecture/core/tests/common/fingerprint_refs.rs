//! Complete descriptive fixtures; no authority or Plan validity is asserted.
use super::api;
use super::api::*;
use alloc::{boxed::Box, format, vec, vec::Vec};
fn base_fragment() -> PlanFragment {
    let value_kind = KindId::from("fixture/byte@1");
    let state = PlannedStateBoundary {
        state_id: StateId::from("retained"),
        gear_id: GearId::from("cell"),
        value_kind: value_kind.clone(),
        initial_value: Some(vec![7]),
        lifetime: StateLifetime::Step,
        retained: None,
        maximum_value_bytes: 1,
        continuation: StateContinuation::ExternallyBounded,
    };
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    PlanFragment {
        completion_policy: api::PlanCompletionPolicy::Live,
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source"),
        checked_plot_id: CheckedPlotId::from("checked"),
        expanded_plot_id: ExpandedPlotId::from("expanded"),
        realization_backs: vec![],
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
        placements: vec![api::planned_gear_from_parts! {
            semantic_contract: Default::default(),
            placement_id: PlacementId::from("placement"),
            gear_id: state.gear_id.clone(),
            kind_id: kind_id("fixture/state"),
            kind_contract_revision: KindIdentity::from("fixture/state@1"),
            execution_profile_id: ExecutionProfileId::from("fixture/state@1"),
            configuration: vec![],
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("state"),
            implementation_id: ImplementationId::from("state@1"),
            artifact_id: ArtifactId::from("state@1"),
            base: None,
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
            inputs: vec![port("next", PortDirection::Input)],
            outputs: vec![port("current", PortDirection::Output)],
            terminal_transductions: Vec::new(),
            host_calls: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![state],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![PlacementId::from("placement")],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
        plan_fragments: vec![],
    }
}
fn line(id: &str) -> AdmittedLine {
    AdmittedLine {
        line_id: LineId::from(id),
        binding: BoundLink {
            binding_id: LinkBindingId::from(id),
            source: LinkEndpoint {
                host_id: HostId::from("source"),
                boot_id: api::BootId::from("source-boot"),
                endpoint_id: LinkEndpointId::from(format!("{id}-source")),
            },
            sink: LinkEndpoint {
                host_id: HostId::from("sink"),
                boot_id: api::BootId::from("sink-boot"),
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
        connection_id: ConnectionId::from("mask/framebuffer-cord"),
        source_placement_id: PlacementId::from("mask/framebuffer-source"),
        source_port_id: PortId::from("surface"),
        sink_placement_id: PlacementId::from("mask/renderer"),
        sink_port_id: PortId::from("surface"),
        value_kind: KindId::from("conduitos.resource/framebuffer@1"),
        resource: Some(Box::new(PlannedResourceConnection {
            contract: ResourcePortContract {
                port_id: PortId::from("surface"),
                class_id: ResourceClassId::from("conduitos.resource/framebuffer@1"),
                ownership: ResourcePortOwnership::Move,
                lifecycle: ResourcePortLifecycle::Play,
                mobility: ResourcePortMobility::HostLocal,
            },
            owner_placement_id: PlacementId::from("mask/framebuffer-source"),
            source_binding: ResourceBinding {
                pool_id: ResourcePoolId::from("framebuffer/surface"),
                class_id: ResourceClassId::from("conduitos.resource/framebuffer@1"),
                units: 1,
                protected: None,
                compute: None,
                content: None,
            },
        })),
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: Vec::new(),
        item_capacity: 1,
        byte_capacity: 1,
    }
}
fn member_offer(kind: &str, revision: &str) -> CapabilityOffer {
    api::capability_offer_from_parts! {
        semantic_contract: Default::default(),
        startup_parameters: vec![FrontStartupParameter {
            name: "peer".into(),
            value_type: "PeerId".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/peer"),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        implementation: api::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/peer-hosted@1"),
            implementation_id: ImplementationId::from("browser/peer-implementation"),
            artifact_id: ArtifactId::from("browser/peer-artifact"),
        },
        inputs: vec![PortDescriptor {
            port_id: api::port_id("recv"),
            value_kind: kind_id("ChatMessage"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: api::port_id("send"),
            value_kind: kind_id("ChatMessage"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 4,
            max_queue_bytes: 1_024,
        },
    }
}

fn pool() -> PlannedSharedPool {
    PlannedSharedPool {
        pool_id: SharedPoolId::from("room/peers"),
        declaration_id: PoolDeclarationId::from("webchat/pool/peers"),
        member_front: member_offer("chat/peer", "chat/peer@1").checked_front(),
        maximum_members: 2,
        member_limits: PoolMemberLimits {
            queue_item_capacity: 4,
            queue_byte_capacity: 1_024,
            sign_item_capacity: 16,
            sign_byte_capacity: 2_048,
        },
        member_sessions_required: false,
        realization_envelope: vec![PoolRealizationEnvelope {
            host_id: HostId::from("browser-host"),
            boot_id: BootId::from("browser-boot"),
            offer_generation: api::OfferGeneration(1),
            capability_id: CapabilityId::from("browser/peer"),
            implementation_id: ImplementationId::from("browser/peer-implementation"),
            artifact_id: ArtifactId::from("browser/peer-artifact"),
            member_capacity: 2,
            resources: vec![],
            admitted_lines: vec![],
        }],
        selection_policy:
            api::SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        admission_authority: AuthorityGrantId::from("grant/admit-room-peer"),
        consumers: vec![PlacementId::from("room"), PlacementId::from("peer-router")],
    }
}

pub fn populated_fragment() -> PlanFragment {
    let mut f = base_fragment();
    f.source_document_id = "source\0é".into();
    f.offer_generation = OfferGeneration(u64::MAX);
    f.realization_backs = vec![PlotBack {
        invocation_path: "invocation".into(),
        kind_id: "kind".into(),
        kind_contract_revision: "revision".into(),
        source_document_id: "source".into(),
        checked_plot_id: "checked".into(),
    }];
    let binding = ResourceBinding {
        pool_id: "lane".into(),
        class_id: "lane-class".into(),
        units: u32::MAX,
        protected: None,
        compute: None,
        content: None,
    };
    f.execution_regions = vec![ExecutionRegion {
        region_id: "region".into(),
        admitted_placements: vec!["placement".into()],
        execution_profile_id: "profile".into(),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: u32::MAX,
        lane_resource: binding,
        lane_base_id: "lane-base".into(),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: u32::MAX,
            timer_slots: u32::MAX,
            cord_item_capacity: u32::MAX,
            cord_byte_capacity: u32::MAX,
            mandatory_sign_items: u16::MAX,
            mandatory_sign_bytes: u32::MAX,
        },
        preemption_required: true,
        isolation_required: true,
    }];
    f.execution_fusions = vec![PlannedFusion {
        fusion_id: "fusion".into(),
        execution_profile_id: "profile".into(),
        implementation_id: "implementation".into(),
        artifact_id: "artifact".into(),
        preserved_placements: vec!["placement".into()],
        preserved_connections: vec!["cord".into()],
    }];
    let mut cord = connection();
    cord.selected_line = Some(Box::new(line("selected")));
    cord.admitted_lines = vec![line("first"), line("second")];
    cord.abnormal_kind = Some("abnormal\0é".into());
    cord.item_capacity = u16::MAX;
    cord.byte_capacity = u32::MAX;
    f.connections = vec![cord];
    f.fore_ports = vec![PlannedForePort {
        front_port_id: "front".into(),
        direction: PortDirection::Input,
        placement_id: "placement".into(),
        gear_port_id: "next".into(),
        value_kind: "kind".into(),
        value_contract: None,
        abnormal_kind: Some("abnormal".into()),
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: u16::MAX,
        byte_capacity: u32::MAX,
        selected_line: Some(Box::new(line("fore-line"))),
    }];
    let mut pool = pool();
    pool.realization_envelope[0].resources = vec![ResourceBinding {
        pool_id: "pool".into(),
        class_id: "class".into(),
        units: u32::MAX,
        protected: None,
        compute: Some(ComputeReservation {
            selected_lanes: u32::MAX,
            service_guarantee: ComputeServiceGuarantee::Reserved,
            architecture_base_id: "architecture".into(),
            architecture_base_kind: ArchitectureBaseKind::HostedOs,
            topology_group_id: Some("topology".into()),
            performance_class: Some("performance".into()),
            nominal_clock_hz: Some(u64::MAX),
        }),
        content: None,
    }];
    pool.realization_envelope[0].admitted_lines = vec![line("pool-line")];
    f.shared_pools = vec![pool];
    f.startup_dependencies = vec![StartupDependency {
        prerequisite_placement_id: "first".into(),
        dependent_placement_id: "second".into(),
    }];
    f.expected_terminals = vec![
        ExpectedTerminal::PlacementCompleted("placement".into()),
        ExpectedTerminal::ConnectionCompleted("cord".into()),
        ExpectedTerminal::PlanCompleted,
    ];
    f.expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared("placement".into()),
        ExpectedSign::PlacementTerminal("placement".into()),
        ExpectedSign::ConnectionTerminal("cord".into()),
        ExpectedSign::PlanTerminal,
    ];
    f.sign_storage_budget = SignStorageBudget {
        item_capacity: u16::MAX,
        byte_capacity: u32::MAX,
    };
    f
}
