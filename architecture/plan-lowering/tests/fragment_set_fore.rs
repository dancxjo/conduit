use conduit_core::{
    ConnectionTrack, DeliveryPressurePolicy, PlannedForePort, PortDirection, PortTemporal,
};
use conduit_kernel::CordEndpoint;
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set, FragmentSetBounds, FragmentSetError},
    lowering::FIXED_KERNEL_STORAGE_PROFILE,
};

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn local_fore_plan(source: &str) -> conduit_core::Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.sign_storage_budget =
        conduit_core::mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    fragment.source_document_id = conduit_core::SourceDocumentId::from(source);
    fragment.fore_ports = [
        ("commands", "next", PortDirection::Input),
        ("states", "current", PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| PlannedForePort {
        front_port_id: conduit_core::port_id(front),
        direction,
        placement_id: conduit_core::PlacementId::from("placement"),
        gear_port_id: conduit_core::port_id(gear),
        value_kind: conduit_core::kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    })
    .collect();
    common::seal(fragment)
}

#[test]
fn selected_fore_line_still_refuses_local_fragment_set() {
    use conduit_core::*;
    let first = local_fore_plan("line-source");
    let mut fragment = first.fragments[0].clone();
    fragment.fore_ports[1].selected_line = Some(AdmittedLine {
        line_id: LineId::from("remote-line"),
        binding: BoundLink {
            binding_id: LinkBindingId::from("remote-binding"),
            source: LinkEndpoint {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                endpoint_id: LinkEndpointId::from("local-output"),
            },
            sink: LinkEndpoint {
                host_id: HostId::from("other-host"),
                boot_id: BootId::from("other-boot"),
                endpoint_id: LinkEndpointId::from("remote-input"),
            },
            base: BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            base_instance_id: BaseInstanceId::from("remote-instance"),
            credential: LinkCredentialReference::Opaque(CredentialReferenceId::from("credential")),
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 1,
                maximum_buffered_bytes: 1,
                maximum_frame_bytes: 2,
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
    });
    let plan = common::seal(fragment);
    assert!(verify_plan(&plan));
    assert!(matches!(
        lower_local_fragment_set(
            &[&plan.fragments[0]],
            FIXED_KERNEL_STORAGE_PROFILE,
            FragmentSetBounds {
                fragments: 1,
                nodes: 1,
                cords: 2,
                queue_slots: 2,
                value_bytes: 2,
                sign_items: 16,
                sign_bytes: 1024,
            }
        ),
        Err(FragmentSetError::RemoteUnsupported)
    ));
}

#[test]
fn local_fore_endpoints_reindex_without_erasing_partition_plan_identity() {
    let first = local_fore_plan("first");
    let second = local_fore_plan("second");
    assert!(conduit_core::verify_plan(&first));
    assert!(conduit_core::verify_plan(&second));
    let lowered = lower_local_fragment_set(
        &[&first.fragments[0], &second.fragments[0]],
        FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 2,
            nodes: 2,
            cords: 4,
            queue_slots: 4,
            value_bytes: 4,
            sign_items: 16,
            sign_bytes: 1024,
        },
    )
    .unwrap();
    assert_eq!(lowered.nodes, 2);
    assert_eq!(lowered.cords, 4);
    for (index, (original, part)) in [first, second].iter().zip(&lowered.partitions).enumerate() {
        assert_eq!(part.identity.plan_id, original.plan_id);
        assert_eq!(part.identity.fragment_id, original.fragments[0].fragment_id);
        assert!(part.remote_endpoints.is_empty());
        for (position, fore) in part.fore_ports.iter().enumerate() {
            let expected = (2 * index + position) as u16;
            assert_eq!(fore.endpoint.0, expected);
            assert_eq!(fore.cord.0, expected);
            let identity = &part.identity.fore_endpoints[position];
            assert_eq!(identity.endpoint, fore.endpoint);
            assert_eq!(identity.cord, fore.cord);
            let cord = &part.cords[position].spec;
            assert_eq!(cord.cord, fore.cord);
            match fore.direction {
                PortDirection::Input => {
                    assert_eq!(cord.source, CordEndpoint::Remote(fore.endpoint))
                }
                PortDirection::Output => assert_eq!(cord.sink, CordEndpoint::Remote(fore.endpoint)),
            }
        }
        let output = &part.fore_ports[1];
        assert!(part
            .routes
            .iter()
            .flat_map(|route| &route.targets)
            .any(|target| {
                target.cord == output.cord && target.sink == CordEndpoint::Remote(output.endpoint)
            }));
    }
}
