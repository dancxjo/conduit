//! Optional storage must preserve the complete historical wire.
use conduit_core::*;
use serde::Serialize;
#[path = "common/sealed_state.rs"]
mod fixture;
#[derive(Serialize)]
struct OriginalConnection {
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub source_port_id: PortId,
    pub sink_placement_id: PlacementId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    /// Resource authority contract and exact admitted source binding. The
    /// opaque bearer remains local to the issuing capability table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<PlannedResourceConnection>,
    /// Exact typed semantic abnormal truth promised by the source Fore port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abnormal_kind: Option<KindId>,
    /// Exact semantic track carried by this Cord.
    #[serde(default)]
    pub track: ConnectionTrack,
    #[serde(default)]
    pub temporal: PortTemporal,
    #[serde(default)]
    pub pressure_policy: DeliveryPressurePolicy,
    /// Initially selected exact Line. Local Cords have no Line.
    #[serde(default)]
    pub selected_line: Option<AdmittedLine>,
    /// Exact ordered permissible Lines. Runtime may select only from this
    /// immutable set; availability remains outside the plan as Signs.
    #[serde(default)]
    pub admitted_lines: Vec<AdmittedLine>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

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

#[test]
fn absent_and_present_optional_records_keep_original_wire_and_identity() {
    for resource in [false, true] {
        for remote in [false, true] {
            let mut current = connection();
            if !resource {
                current.resource = None;
            }
            current.selected_line = remote.then(|| Box::new(line("line-a")));
            let original = OriginalConnection {
                connection_id: current.connection_id.clone(),
                source_placement_id: current.source_placement_id.clone(),
                source_port_id: current.source_port_id.clone(),
                sink_placement_id: current.sink_placement_id.clone(),
                sink_port_id: current.sink_port_id.clone(),
                value_kind: current.value_kind.clone(),
                abnormal_kind: current.abnormal_kind.clone(),
                track: current.track,
                temporal: current.temporal,
                pressure_policy: current.pressure_policy,
                admitted_lines: current.admitted_lines.clone(),
                item_capacity: current.item_capacity,
                byte_capacity: current.byte_capacity,
                resource: current.resource.as_deref().cloned(),
                selected_line: current.selected_line.as_deref().cloned(),
            };
            let old_json = serde_json::to_vec(&original).unwrap();
            assert_eq!(serde_json::to_vec(&current).unwrap(), old_json);
            assert_eq!(
                postcard::to_allocvec(&current).unwrap(),
                postcard::to_allocvec(&original).unwrap()
            );
            let decoded: PlannedConnection = serde_json::from_slice(&old_json).unwrap();
            assert_eq!(decoded, current);
            let mut fragment = fixture::fragment();
            fragment.connections = vec![current];
            let identity = compute_fragment_id(&fragment);
            fragment.connections = vec![decoded];
            assert_eq!(compute_fragment_id(&fragment), identity);
        }
    }
}
