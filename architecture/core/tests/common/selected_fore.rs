//! Shared storage-census fixture only; references grant no authority or transport.
use conduit_core::*;

pub fn line(fragment: &PlanFragment, credential: bool, grant: bool) -> AdmittedLine {
    let mut spare = String::with_capacity(4096);
    spare.push_str("fixture/selected-fore-line");
    AdmittedLine {
        line_id: LineId(spare),
        binding: BoundLink {
            binding_id: "fixture/selected-binding".into(),
            source: LinkEndpoint {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                endpoint_id: "fixture/source".into(),
            },
            sink: LinkEndpoint {
                host_id: "fixture/peer".into(),
                boot_id: "fixture/peer-boot".into(),
                endpoint_id: "fixture/sink".into(),
            },
            base: "conduit.base/websocket-rfc6455@1".into(),
            base_instance_id: "fixture/instance".into(),
            credential: if credential {
                LinkCredentialReference::Opaque("fixture/credential".into())
            } else {
                LinkCredentialReference::None
            },
            authority: if grant {
                LinkAuthorityReference::Grant("fixture/grant-reference".into())
            } else {
                LinkAuthorityReference::ProcessOwned
            },
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
    }
}
pub fn plan(mut fragment: PlanFragment) -> Plan {
    fragment.states.clear();
    fragment.fore_ports = [
        ("commands", "next", PortDirection::Input),
        ("states", "current", PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| PlannedForePort {
        front_port_id: port_id(front),
        direction,
        placement_id: "placement".into(),
        gear_port_id: port_id(gear),
        value_kind: kind_id("fixture/byte@1"),
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
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
