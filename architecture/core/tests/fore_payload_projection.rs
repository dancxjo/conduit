//! A payload export does not silently promise or consume the sibling ! track.
use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod common;
fn fragment(direction: PortDirection, abnormal: Option<&str>) -> PlanFragment {
    let mut fragment = common::fragment();
    let placement = &mut fragment.placements[0];
    let port = match direction {
        PortDirection::Input => &mut placement.inputs[0],
        PortDirection::Output => &mut placement.outputs[0],
    };
    port.abnormal_kind = Some(kind_id("fixture/fault@1"));
    fragment.fore_ports.push(PlannedForePort {
        front_port_id: port_id("value"),
        direction,
        placement_id: placement.placement_id.clone(),
        gear_port_id: port.port_id.clone(),
        value_kind: port.value_kind.clone(),
        value_contract: None,
        abnormal_kind: abnormal.map(kind_id),
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    });
    fragment
}
#[test]
fn output_payload_can_be_exported_without_promising_the_separate_abnormal_track() {
    let fragment = fragment(PortDirection::Output, None);
    assert!(verify_plan(&common::seal(fragment.clone())));
    assert_eq!(
        fragment.placements[0].outputs[0]
            .abnormal_kind
            .as_ref()
            .unwrap()
            .as_str(),
        "fixture/fault@1"
    );
    assert!(fragment.fore_ports[0].abnormal_kind.is_none());
    for mutation in 0..3 {
        let mut forged = fragment.clone();
        match mutation {
            0 => forged.fore_ports[0].value_kind = kind_id("fixture/other-value@1"),
            1 => forged.fore_ports[0].temporal = PortTemporal::Current,
            _ => forged.fore_ports[0].abnormal_kind = Some(kind_id("fixture/other-fault@1")),
        }
        assert!(!verify_plan(&common::seal(forged)));
    }
}
#[test]
fn explicitly_promised_abnormal_output_contract_remains_exact() {
    assert!(verify_plan(&common::seal(fragment(
        PortDirection::Output,
        Some("fixture/fault@1")
    ))));
    assert!(!verify_plan(&common::seal(fragment(
        PortDirection::Output,
        Some("fixture/other-fault@1")
    ))));
}
#[test]
fn input_payload_requires_the_complete_declared_input_contract() {
    assert!(!verify_plan(&common::seal(fragment(
        PortDirection::Input,
        None
    ))));
    assert!(verify_plan(&common::seal(fragment(
        PortDirection::Input,
        Some("fixture/fault@1")
    ))));
    assert!(!verify_plan(&common::seal(fragment(
        PortDirection::Input,
        Some("fixture/other-fault@1")
    ))));
}

#[test]
fn separate_abnormal_fore_still_requires_its_actual_terminal_type() {
    let mut fragment = fragment(PortDirection::Output, None);
    let mut terminal = fragment.fore_ports[0].clone();
    terminal.front_port_id = port_id("fault");
    terminal.track = ConnectionTrack::AbnormalTerminal;
    terminal.value_kind = kind_id("fixture/fault@1");
    fragment.fore_ports.push(terminal);
    assert!(verify_plan(&common::seal(fragment.clone())));
    fragment.fore_ports[1].value_kind = kind_id("fixture/other-fault@1");
    assert!(!verify_plan(&common::seal(fragment)));
}
