use super::*;
use conduit_core::{AssignedIdentity, LineContinuation};
fn installation() -> PhysicalForeInstallation<'static> {
    PhysicalForeInstallation {
        body_id: "body",
        configuration_id: "uart-config",
        implementation_id: "esp32/root-uart-fore@1",
        session_contract: "physical-fore@1",
    }
}
fn local() -> PhysicalForePeerFacts<'static> {
    PhysicalForePeerFacts {
        host_id: "mcu",
        boot_id: "mcu-boot",
        offer_generation: 3,
    }
}
fn peer() -> PhysicalForePeerFacts<'static> {
    PhysicalForePeerFacts {
        host_id: "linux",
        boot_id: "linux-boot",
        offer_generation: 4,
    }
}
fn limits() -> LinkLimits {
    LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: 64,
        maximum_buffered_bytes: 64,
        maximum_frame_bytes: 116,
    }
}
fn contract() -> LineContract {
    LineContract {
        scope: LineScope::PointToPoint,
        traffic_shape: LineTrafficShape::Message,
        duplex: LineDuplex::FullDuplex,
        ordering: LineOrdering::Ordered,
        reliability: LineReliability::Reliable,
        continuation: LineContinuation::None,
        security: LineSecurity::PhysicalPossession,
    }
}
fn fore(direction: AssignedForeDirection) -> AssignedForeRequirement {
    AssignedForeRequirement::new(
        2,
        3,
        direction,
        AssignedIdentity::from_text("read_index"),
        [9; 32],
    )
}
fn make(
    i: PhysicalForeInstallation<'_>,
    l: PhysicalForePeerFacts<'_>,
    p: PhysicalForePeerFacts<'_>,
    limits: LinkLimits,
    c: LineContract,
) -> PhysicalForeLine {
    describe_physical_fore_line(i, l, p, &fore(AssignedForeDirection::Input), limits, c).unwrap()
}
#[test]
fn independently_reproduced_legs_share_carrier_session_but_pin_exact_fore() {
    let a = make(installation(), local(), peer(), limits(), contract());
    let b = make(installation(), local(), peer(), limits(), contract());
    assert_eq!(a.line, b.line);
    assert_eq!(a.session_id, b.session_id);
    assert_eq!(
        a.session_id,
        "dd057b53b652ed9f43ec1a66bd0347972ab65d7e549f0a7f81b5466138db4701"
    );
    assert_eq!(a.line.binding.source.host_id.as_str(), "linux");
    assert_eq!(a.line.binding.sink.host_id.as_str(), "mcu");
    let output = describe_physical_fore_line(
        installation(),
        local(),
        peer(),
        &fore(AssignedForeDirection::Output),
        limits(),
        contract(),
    )
    .unwrap();
    assert_eq!(a.session_id, output.session_id);
    assert_ne!(a.line.line_id, output.line.line_id);
    assert_ne!(a.line.binding.binding_id, output.line.binding.binding_id);
    assert_ne!(
        a.line.binding.sink.endpoint_id,
        output.line.binding.source.endpoint_id
    );
    assert_eq!(output.line.binding.source.host_id.as_str(), "mcu");
    assert!(a.session_id.len() <= 160 && a.line.line_id.as_str().len() <= 160);
    let other = AssignedForeRequirement::new(
        2,
        3,
        AssignedForeDirection::Input,
        AssignedIdentity::from_text("read_result"),
        [8; 32],
    );
    let other = describe_physical_fore_line(
        installation(),
        local(),
        peer(),
        &other,
        limits(),
        contract(),
    )
    .unwrap();
    assert_eq!(a.session_id, other.session_id);
    assert_ne!(a.line, other.line);
}
#[test]
fn installed_identity_mutations_change_session_and_line() {
    let original = make(installation(), local(), peer(), limits(), contract());
    let mut variants = alloc::vec::Vec::new();
    for field in 0..4 {
        let mut i = installation();
        match field {
            0 => i.body_id = "other",
            1 => i.configuration_id = "other",
            2 => i.implementation_id = "rp2040/root-usb-fore@1",
            _ => i.session_contract = "other",
        }
        variants.push(make(i, local(), peer(), limits(), contract()));
    }
    for field in 0..6 {
        let (mut l, mut p) = (local(), peer());
        match field {
            0 => l.host_id = "other",
            1 => l.boot_id = "other",
            2 => l.offer_generation += 1,
            3 => p.host_id = "other",
            4 => p.boot_id = "other",
            _ => p.offer_generation += 1,
        }
        variants.push(make(installation(), l, p, limits(), contract()));
    }
    for field in 0..4 {
        let mut n = limits();
        match field {
            0 => n.maximum_in_flight_items += 1,
            1 => n.maximum_payload_bytes -= 1,
            2 => n.maximum_buffered_bytes += 1,
            _ => n.maximum_frame_bytes += 1,
        }
        variants.push(make(installation(), local(), peer(), n, contract()));
    }
    let mut c = contract();
    c.continuation = LineContinuation::BoundedSessionReconciliation;
    variants.push(make(installation(), local(), peer(), limits(), c));
    for changed in variants {
        assert_ne!(original.session_id, changed.session_id);
        assert_ne!(original.line, changed.line);
    }
}
#[test]
fn invalid_facts_contracts_and_frame_extents_refuse() {
    let f = fore(AssignedForeDirection::Input);
    let check = |i, l, p, n, c| describe_physical_fore_line(i, l, p, &f, n, c).is_err();
    let mut i = installation();
    i.body_id = "";
    assert!(check(i, local(), peer(), limits(), contract()));
    let long = "x".repeat(161);
    i = installation();
    i.body_id = &long;
    assert!(check(i, local(), peer(), limits(), contract()));
    assert!(check(
        installation(),
        local(),
        local(),
        limits(),
        contract()
    ));
    for field in 0..4 {
        let mut n = limits();
        match field {
            0 => n.maximum_in_flight_items = 0,
            1 => n.maximum_payload_bytes = 0,
            2 => n.maximum_frame_bytes -= 1,
            _ => {
                n.maximum_payload_bytes = u32::MAX;
                n.maximum_buffered_bytes = u32::MAX;
                n.maximum_frame_bytes = u32::MAX
            }
        }
        assert!(check(installation(), local(), peer(), n, contract()));
    }
    for field in 0..6 {
        let mut c = contract();
        match field {
            0 => c.scope = LineScope::Machine,
            1 => c.traffic_shape = LineTrafficShape::ByteStream,
            2 => c.security = LineSecurity::PlaintextNetwork,
            3 => c.ordering = LineOrdering::Unordered,
            4 => c.reliability = LineReliability::BestEffort,
            _ => c.duplex = LineDuplex::Simplex,
        }
        assert!(check(installation(), local(), peer(), limits(), c));
    }
}
