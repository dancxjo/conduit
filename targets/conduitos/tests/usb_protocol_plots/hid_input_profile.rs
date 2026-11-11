//! HID selection consumes the existing checked configuration topology.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn leaf(ty: &StructuredInfoType, name: &str, value: &[u8]) -> StructuredFieldValue {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(field(ty, name), value.to_vec()).unwrap(),
    )
    .unwrap()
}
fn record(ty: &StructuredInfoType, fields: Vec<StructuredFieldValue>) -> StructuredInfoValue {
    StructuredInfoValue::record(ty.clone(), fields).unwrap()
}
fn element(ty: &StructuredInfoType) -> StructuredInfoType {
    let StructuredInfoTypeShape::Collection { element, .. } = ty.shape() else {
        panic!("collection")
    };
    element.clone()
}
#[derive(Clone, Copy)]
struct Fixture {
    class: u8,
    boot: bool,
    packet: u64,
    attributes: u8,
    low: bool,
    high: bool,
    interfaces: u64,
    endpoints: u64,
}
fn input(ty: &StructuredInfoType, case: Fixture) -> Vec<u8> {
    let topology = field(ty, "topology");
    let interfaces = field(&topology, "interfaces");
    let endpoints = field(&topology, "endpoints");
    let interface = element(&interfaces);
    let endpoint = element(&endpoints);
    let interface_values = (0..4)
        .map(|i| {
            record(
                &interface,
                vec![
                    leaf(&interface, "number", &[if i == 1 { 11 } else { 7 }]),
                    leaf(&interface, "alternate", &[0]),
                    leaf(
                        &interface,
                        "endpoints",
                        &[if i == 0 {
                            3
                        } else if i == 1 {
                            1
                        } else {
                            0
                        }],
                    ),
                    leaf(&interface, "class", &[if i == 1 { case.class } else { 2 }]),
                    leaf(&interface, "subclass", &[u8::from(i == 1 && case.boot)]),
                    leaf(&interface, "protocol", &[u8::from(i == 1 && case.boot)]),
                ],
            )
        })
        .collect();
    let endpoint_values = (0..8)
        .map(|i| {
            record(
                &endpoint,
                vec![
                    leaf(
                        &endpoint,
                        "interface_slot",
                        &if i == 3 { 1_u64 } else { 0 }.to_le_bytes(),
                    ),
                    leaf(
                        &endpoint,
                        "address",
                        &[if i == 3 { 0x85 } else { 0x81 + i as u8 }],
                    ),
                    leaf(&endpoint, "attributes", &[case.attributes]),
                    leaf(&endpoint, "packet_field", &case.packet.to_le_bytes()),
                    leaf(&endpoint, "interval", &[4]),
                ],
            )
        })
        .collect();
    let topology_value = record(
        &topology,
        vec![
            leaf(&topology, "configuration_value", &[3]),
            leaf(&topology, "attributes", &[0x80]),
            leaf(&topology, "maximum_power_field", &[50]),
            leaf(&topology, "distinct_interfaces", &2_u64.to_le_bytes()),
            leaf(&topology, "interface_count", &case.interfaces.to_le_bytes()),
            leaf(&topology, "endpoint_count", &case.endpoints.to_le_bytes()),
            StructuredFieldValue::new(
                "interfaces",
                StructuredInfoValue::collection(interfaces, interface_values).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "endpoints",
                StructuredInfoValue::collection(endpoints, endpoint_values).unwrap(),
            )
            .unwrap(),
        ],
    );
    record(
        ty,
        vec![
            StructuredFieldValue::new("topology", topology_value).unwrap(),
            leaf(ty, "low-speed", &[u8::from(case.low)]),
            leaf(ty, "high-speed", &[u8::from(case.high)]),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
#[test]
fn hid_selection_uses_interface_membership_direction_packet_geometry_and_link_profile() {
    let source = conduitos::protocol_source::PreparedProtocolSource::prepare(
        conduitos::protocol_source::usb_hid_configuration_package().unwrap(),
    )
    .unwrap();
    let programs = [
        "usb-hid-input-interface",
        "usb-hid-input-candidates",
        "usb-hid-input-select",
    ]
    .map(|name| {
        let expanded = source.expand(name).unwrap().expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    });
    let mut owners = programs
        .each_ref()
        .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap());
    let good = Fixture {
        class: 3,
        boot: false,
        packet: 64,
        attributes: 3,
        low: false,
        high: false,
        interfaces: 2,
        endpoints: 4,
    };
    for (case, tag) in [
        (good, "selected"),
        (
            Fixture {
                boot: true,
                packet: 8,
                ..good
            },
            "selected",
        ),
        (
            Fixture {
                boot: true,
                packet: 16,
                ..good
            },
            "unsupported-endpoint",
        ),
        (Fixture { packet: 32, ..good }, "selected"),
        (Fixture { class: 2, ..good }, "no-hid-interface"),
        (
            Fixture {
                attributes: 2,
                ..good
            },
            "unsupported-endpoint",
        ),
        (Fixture { packet: 65, ..good }, "unsupported-endpoint"),
        (Fixture { low: true, ..good }, "unsupported-endpoint"),
        (
            Fixture {
                low: true,
                packet: 8,
                ..good
            },
            "selected",
        ),
        (Fixture { high: true, ..good }, "unsupported-link"),
        (
            Fixture {
                interfaces: 5,
                ..good
            },
            "malformed",
        ),
        (
            Fixture {
                endpoints: 9,
                ..good
            },
            "malformed",
        ),
    ] {
        let input = input(&programs[0].input_type, case);
        let [interface, candidates, select] = &mut owners;
        let allocations = super::allocation::allocations(|| {
            let current = interface.evaluate(&input).unwrap();
            let current = candidates.evaluate(current).unwrap();
            let bytes = select.evaluate(current).unwrap();
            let result = validate_canonical_structured_value(bytes).unwrap();
            let payload = result.variant_payload(tag).unwrap().expect(tag);
            if tag == "selected" {
                assert_eq!(
                    payload
                        .record_field("boot-keyboard")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/bool")
                        .unwrap(),
                    [u8::from(case.boot)]
                );
                for (name, expected) in [
                    ("configuration-value", 3),
                    ("interface-number", 11),
                    ("endpoint", 5),
                ] {
                    assert_eq!(
                        payload
                            .record_field(name)
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u8")
                            .unwrap(),
                        [expected]
                    );
                }
                assert_eq!(
                    payload
                        .record_field("maximum-packet")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    case.packet.to_le_bytes()
                );
                assert_eq!(
                    payload
                        .record_field("interval-ms")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    4_u64.to_le_bytes()
                );
            }
        });
        assert_eq!(allocations, 0);
    }
}
