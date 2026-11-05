use super::*;
use conduit_core::{
    CheckedValueContract, ConnectionTrack, PortTemporal, QuantityValueProfile, ValueConstraint,
};
use conduit_kernel::{CordId, RemoteEndpointId};
fn port() -> LoweredForePort {
    LoweredForePort {
        front_port_id: "request".into(),
        direction: PortDirection::Input,
        track: ConnectionTrack::Payload,
        endpoint: RemoteEndpointId(0),
        cord: CordId(0),
        value_kind: "test/command".into(),
        value_contract: Some(CheckedValueContract::new("test/command".into(), 1, vec![]).unwrap()),
        quantity_profile: QuantityValueProfile::Decimal,
        abnormal_kind: None,
        temporal: PortTemporal::Flow { closes: true },
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    }
}
fn route<'a>(
    port: LoweredForePort,
    supplied: &'a [ExternalForeInput],
    sequential: bool,
) -> BodyForeRoute<'a> {
    let inputs = PreparedForeInputs::prepare(&[&port], supplied, sequential).unwrap();
    BodyForeRoute {
        inputs,
        ports: vec![port],
        output_slots: Vec::new(),
        deliveries: Vec::new(),
        pending_output: None,
        started: false,
        acknowledged_inputs: VecDeque::new(),
    }
}
#[test]
fn one_shot_input_has_owned_metadata_without_becoming_a_tick_receipt() {
    let supplied = [ExternalForeInput {
        front_port_id: "request".into(),
        track: ConnectionTrack::Payload,
        bytes: vec![1],
    }];
    let route = route(port(), &supplied, false);
    let bytes = route.local_one_shot_backing_bytes().unwrap();
    assert!(bytes >= core::mem::size_of::<LoweredForePort>());
    assert!(route.tick_backing_bytes().is_none());
    assert_eq!(route.finite_one_shot_backing_bytes(), Some(bytes));
    assert!(!route.started);
    assert_eq!(supplied[0].bytes, [1]);
}
#[test]
fn nested_constraints_and_spare_capacity_are_charged_as_owned_backing() {
    let supplied = [ExternalForeInput {
        front_port_id: "request".into(),
        track: ConnectionTrack::Payload,
        bytes: vec![1],
    }];
    let mut route = route(port(), &supplied, false);
    let before = route.local_one_shot_backing_bytes().unwrap();
    let mut members = Vec::with_capacity(7);
    let mut member = Vec::with_capacity(123);
    member.push(1);
    members.push(member);
    let mut constraints = Vec::with_capacity(5);
    let extra = constraints.capacity() * core::mem::size_of::<ValueConstraint>()
        + members.capacity() * core::mem::size_of::<Vec<u8>>()
        + members[0].capacity();
    constraints.push(ValueConstraint::CanonicalMembership {
        members,
        negated: false,
    });
    route.ports[0].value_contract.as_mut().unwrap().constraints = constraints;
    assert_eq!(
        route.local_one_shot_backing_bytes(),
        before.checked_add(extra)
    );
    assert!(route.tick_backing_bytes().is_none());
    assert!(route.finite_one_shot_backing_bytes().is_none());
}
#[test]
fn sequential_ingress_is_not_silently_counted_as_one_shot() {
    let supplied = [ExternalForeInput {
        front_port_id: "request".into(),
        track: ConnectionTrack::Payload,
        bytes: vec![1],
    }];
    let route = route(port(), &supplied, true);
    assert!(route.local_one_shot_backing_bytes().is_none());
    assert!(route.tick_backing_bytes().is_none());
    assert!(route.finite_one_shot_backing_bytes().is_none());
}

#[test]
fn owned_quantity_metadata_does_not_qualify_its_validation_workspace() {
    let supplied = [ExternalForeInput {
        front_port_id: "request".into(),
        track: ConnectionTrack::Payload,
        bytes: vec![1],
    }];
    let mut route = route(port(), &supplied, false);
    route.ports[0].value_kind = conduit_core::QUANTITY_INFO_ID.into();
    assert!(route.local_one_shot_backing_bytes().is_some());
    assert!(route.finite_one_shot_backing_bytes().is_none());
}
