//! A named device fixture drives ordinary Source/kernel enumeration, not hardware.
use conduit_core::*;
use conduitos::{
    protocol_source::*,
    usb_base::{
        control_contract::ControlContract, control_request::ControlTransferRequest,
        control_result::PreparedControlResultEncoder, hid_source_kernel::PreparedUsbSourceKernel,
    },
};
use std::collections::BTreeMap;
struct Exchange {
    setup: [u8; 8],
    address: u8,
    packet: u8,
    translator: u8,
    translator_hub: u8,
    reply: Vec<u8>,
}
fn exchange(word: u64, address: u8, packet: u8, translator: u8, reply: &[u8]) -> Exchange {
    Exchange {
        setup: word.to_le_bytes(),
        address,
        packet,
        translator,
        translator_hub: if translator == 0 { 0 } else { 1 },
        reply: reply.to_vec(),
    }
}
// First 256 bytes of the 426-byte Waveshare WS170120 HID report descriptor
// captured on the host. The hub plot requests exactly this prefix.
const WAVESHARE_REPORT_PREFIX: &[u8; 256] = &[
    0x05, 0x0d, 0x09, 0x04, 0xa1, 0x01, 0x85, 0x01, 0x09, 0x22, 0xa1, 0x02, 0x09, 0x42, 0x15, 0x00,
    0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x95, 0x07, 0x81, 0x01, 0x75, 0x08, 0x09, 0x51,
    0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x26, 0x00, 0x04, 0x75, 0x10, 0x55, 0x0e, 0x65, 0x11, 0x09,
    0x30, 0x35, 0x00, 0x46, 0x79, 0x08, 0x81, 0x02, 0x26, 0x58, 0x02, 0x46, 0x4c, 0x05, 0x09, 0x31,
    0x81, 0x02, 0x05, 0x0d, 0x09, 0x48, 0x81, 0x02, 0xc0, 0x09, 0x22, 0xa1, 0x02, 0x09, 0x42, 0x15,
    0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x95, 0x07, 0x81, 0x01, 0x75, 0x08, 0x09,
    0x51, 0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x26, 0x00, 0x04, 0x75, 0x10, 0x55, 0x0e, 0x65, 0x11,
    0x09, 0x30, 0x35, 0x00, 0x46, 0x79, 0x08, 0x81, 0x02, 0x26, 0x58, 0x02, 0x46, 0x4c, 0x05, 0x09,
    0x31, 0x81, 0x02, 0x05, 0x0d, 0x09, 0x48, 0x81, 0x02, 0xc0, 0x09, 0x22, 0xa1, 0x02, 0x09, 0x42,
    0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x95, 0x07, 0x81, 0x01, 0x75, 0x08,
    0x09, 0x51, 0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x26, 0x00, 0x04, 0x75, 0x10, 0x55, 0x0e, 0x65,
    0x11, 0x09, 0x30, 0x35, 0x00, 0x46, 0x79, 0x08, 0x81, 0x02, 0x26, 0x58, 0x02, 0x46, 0x4c, 0x05,
    0x09, 0x31, 0x81, 0x02, 0x05, 0x0d, 0x09, 0x48, 0x81, 0x02, 0xc0, 0x09, 0x22, 0xa1, 0x02, 0x09,
    0x42, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x95, 0x07, 0x81, 0x01, 0x75,
    0x08, 0x09, 0x51, 0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x26, 0x00, 0x04, 0x75, 0x10, 0x55, 0x0e,
    0x65, 0x11, 0x09, 0x30, 0x35, 0x00, 0x46, 0x79, 0x08, 0x81, 0x02, 0x26, 0x58, 0x02, 0x46, 0x4c,
];

fn fixture() -> Vec<Exchange> {
    let hub_config = [
        9, 2, 25, 0, 1, 1, 0, 0xe0, 0, 9, 4, 0, 0, 1, 9, 0, 0, 0, 7, 5, 0x81, 3, 1, 0, 12,
    ];
    let input_config = [
        9, 2, 50, 0, 2, 3, 0, 0x80, 50, 9, 4, 0, 0, 0, 2, 0, 0, 0, 9, 4, 1, 0, 2, 3, 0, 0, 0, 9,
        0x21, 0x11, 1, 0, 1, 0x22, 0xaa, 1, 7, 5, 0x83, 3, 64, 0, 1, 7, 5, 4, 3, 64, 0, 1,
    ];
    vec![
        exchange(0x0008000001000680, 0, 64, 0, &[18, 1, 0, 2, 9, 0, 2, 64]),
        exchange(0x0000000000010500, 0, 64, 0, &[]),
        exchange(0x0009000002000680, 1, 64, 0, &hub_config[..9]),
        exchange(0x0019000002000680, 1, 64, 0, &hub_config),
        exchange(0x0000000000010900, 1, 64, 0, &[]),
        exchange(
            0x00090000290006a0,
            1,
            64,
            0,
            &[9, 41, 2, 9, 0, 1, 0, 0, 255],
        ),
        exchange(0x0000000100080323, 1, 64, 0, &[]),
        exchange(0x0000000200080323, 1, 64, 0, &[]),
        exchange(0x00040001000000a3, 1, 64, 0, &[0, 1, 0, 0]),
        exchange(0x00040002000000a3, 1, 64, 0, &[1, 1, 1, 0]),
        exchange(0x0000000200040323, 1, 64, 0, &[]),
        exchange(0x00040002000000a3, 1, 64, 0, &[3, 1, 16, 0]),
        exchange(0x0000000200140123, 1, 64, 0, &[]),
        exchange(0x0000000200100123, 1, 64, 0, &[]),
        exchange(0x0008000001000680, 0, 8, 2, &[18, 1, 0x10, 1, 0, 0, 0, 64]),
        exchange(0x0000000000030500, 0, 64, 2, &[]),
        exchange(0x0009000002000680, 3, 64, 2, &input_config[..9]),
        exchange(0x0032000002000680, 3, 64, 2, &input_config),
        exchange(0x0000000000030900, 3, 64, 2, &[]),
        exchange(0x0100000122000681, 3, 64, 2, WAVESHARE_REPORT_PREFIX),
    ]
}
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
fn byte(value: ValidatedCanonicalStructuredValue<'_>, name: &str) -> u8 {
    value
        .record_field(name)
        .unwrap()
        .unwrap()
        .primitive_bytes("value/u8")
        .unwrap()[0]
}
#[test]
fn hub_input_policy_runs_complete_bounded_enumeration_without_native_authority_or_allocations() {
    run_fixture(fixture(), &[2, 2, 50, 2], 3, 1);
}
#[test]
fn nested_high_speed_hub_routes_the_same_waveshare_report_through_its_own_translator() {
    let mut tree = nested_hub_prefix();
    for mut item in fixture().into_iter().skip(6) {
        if item.address == 1 {
            item.address = 2;
        } else if item.address == 3 {
            item.address = 10;
        }
        if item.setup == 0x0000000000030500_u64.to_le_bytes() {
            item.setup = 0x00000000000a0500_u64.to_le_bytes();
        }
        if item.translator != 0 {
            item.translator_hub = 2;
        }
        tree.push(item);
    }
    run_fixture(tree, &[2, 2, 50, 2, 2, 50, 2], 10, 2);
}
#[test]
fn empty_child_hub_resumes_the_parent_without_reusing_device_addresses() {
    let mut tree = nested_hub_prefix();
    tree.push(exchange(0x0000000100080323, 2, 64, 0, &[]));
    tree.push(exchange(0x0000000200080323, 2, 64, 0, &[]));
    tree.push(exchange(0x00040001000000a3, 2, 64, 0, &[0, 1, 0, 0]));
    tree.push(exchange(0x00040002000000a3, 2, 64, 0, &[0, 1, 0, 0]));
    tree.extend(fixture().into_iter().skip(9));
    run_fixture(tree, &[2, 2, 50, 2, 2, 50, 2], 3, 1);
}
fn nested_hub_prefix() -> Vec<Exchange> {
    let original = fixture();
    let mut tree: Vec<_> = fixture().into_iter().take(8).collect();
    // Root port 1 contains a high-speed child hub, not an input device.
    tree.push(exchange(0x00040001000000a3, 1, 64, 0, &[1, 5, 1, 0]));
    tree.push(exchange(0x0000000100040323, 1, 64, 0, &[]));
    tree.push(exchange(0x00040001000000a3, 1, 64, 0, &[3, 5, 16, 0]));
    tree.push(exchange(0x0000000100140123, 1, 64, 0, &[]));
    tree.push(exchange(0x0000000100100123, 1, 64, 0, &[]));
    tree.push(exchange(
        0x0008000001000680,
        0,
        64,
        0,
        &[18, 1, 0, 2, 9, 0, 2, 64],
    ));
    tree.push(exchange(0x0000000000020500, 0, 64, 0, &[]));
    // The already addressed hub is configured once, then its descriptor and
    // ports use its own address; default-address traffic remains exclusive.
    tree.push(exchange(0x0009000002000680, 2, 64, 0, &original[2].reply));
    tree.push(exchange(0x0019000002000680, 2, 64, 0, &original[3].reply));
    tree.push(exchange(0x0000000000010900, 2, 64, 0, &[]));
    tree.push(exchange(0x00090000290006a0, 2, 64, 0, &original[5].reply));
    tree
}
fn run_fixture(fixture: Vec<Exchange>, delays: &[u64], expected_address: u8, expected_hub: u8) {
    let package = usb_hub_input_package().unwrap();
    let bytes = serde_json::to_vec(&package).unwrap();
    eprintln!(
        "hub fixture: source_bytes={} package_bytes={} exchanges={}",
        package.source.len(),
        bytes.len(),
        fixture.len()
    );
    drop(package);
    let entry = PreparedProtocolEntry::prepare(&bytes, "usb-hub-input").unwrap();
    let expanded = &entry.expanded().expanded;
    assert!(expanded.gears.len() <= 64);
    assert!(
        !expanded
            .gears
            .iter()
            .any(|g| g.kind_id.as_str() == "machine/usb/control")
    );
    let begin_type = entry.input_schema(&port_id("begin")).unwrap();
    let mut host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/hub-policy-host".into(),
        boot_id: "fixture/hub-policy-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/pure-hub-policy".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry.publish_pure_backs(&mut host).unwrap();
    let hosts = [host];
    let placements = entry.placements(&hosts).unwrap();
    let artifact = entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
    let mut configuration = PreparedPureProtocolPipe::prepare(
        usb_configuration_decode_package().unwrap(),
        "usb-configuration-decode",
        "fixture/hub-policy-host".into(),
        "fixture/hub-policy-boot".into(),
    )
    .unwrap();
    let mut descriptor = PreparedHidDescriptorPipe::prepare(
        "fixture/hub-policy-host".into(),
        "fixture/hub-policy-boot".into(),
    )
    .unwrap();
    let mut run = PreparedUsbSourceKernel::prepare_pure(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    let front = run.kernel_mut().definition().boundary.clone();
    let input = |name: &str| {
        front
            .input_fronts
            .iter()
            .find(|f| f.external_port.port_id.as_str() == name)
            .unwrap()
            .external_port
            .clone()
    };
    let begin = input("begin");
    let received = input("received");
    let woke = input("woke");
    let topology = input("topology");
    let layout_input = input("layout");
    let outputs: Vec<_> = front
        .output_fronts
        .iter()
        .map(|p| p.external_port.clone())
        .collect();
    let mut output: Vec<_> = outputs
        .iter()
        .map(|p| ValuePayload {
            value_kind: p.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let fields = [
        ("hub-address", 1_u8),
        ("first-child-address", 2),
        ("maximum-packet", 64),
    ]
    .into_iter()
    .map(|(name, n)| {
        StructuredFieldValue::new(
            name,
            StructuredInfoValue::leaf(field(&begin_type, name), vec![n]).unwrap(),
        )
        .unwrap()
    })
    .collect();
    let seed = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: StructuredInfoValue::record(begin_type, fields)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    };
    let mut response = ValuePayload {
        value_kind: received.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let pulse = ValuePayload {
        value_kind: woke.value_kind.clone(),
        encoded: vec![],
    };
    let control = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&control).unwrap();
    let mut call = 0;
    let mut wait = 0;
    let mut topology_sequence = 0;
    let mut layout_sequence = 0;
    let mut result = false;
    let mut complete = false;
    let allocations = super::allocation::allocations(|| {
        configuration.start().unwrap();
        descriptor.start().unwrap();
        run.kernel_mut().start().unwrap();
        run.kernel_mut()
            .admit_input(&begin.port_id, 0, &seed)
            .unwrap();
        run.kernel_mut().close_input(&begin.port_id).unwrap();
        for _ in 0..40_000 {
            if !configuration.complete() {
                configuration.step().unwrap();
            }
            if !descriptor.complete() {
                descriptor.step().unwrap();
            }
            if let Some(value) = configuration.result() {
                assert!(matches!(
                    run.kernel_mut()
                        .admit_input(&topology.port_id, topology_sequence, value)
                        .unwrap(),
                    conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
                ));
                topology_sequence += 1;
                configuration.acknowledge();
            }
            if let Some(value) = descriptor.result() {
                assert!(matches!(
                    run.kernel_mut()
                        .admit_input(&layout_input.port_id, layout_sequence, value)
                        .unwrap(),
                    conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
                ));
                layout_sequence += 1;
                descriptor.acknowledge();
            }
            let status = run.kernel_mut().step().unwrap();
            if let Some(request) = run.kernel_mut().next_host_request() {
                assert!(run.service_pure_call(&request).unwrap());
            }
            for (index, port) in outputs.iter().enumerate() {
                if let Some(sequence) = run
                    .kernel_mut()
                    .output_into(&port.port_id, &mut output[index])
                    .unwrap()
                {
                    match port.port_id.as_str() {
                        "exchange" => {
                            let expected = fixture.get(call).expect("no invented request");
                            let value = validate_canonical_structured_value(&output[index].encoded)
                                .unwrap();
                            assert_eq!(byte(value, "address"), expected.address, "call {call}");
                            assert_eq!(
                                byte(value, "maximum-packet"),
                                expected.packet,
                                "call {call}"
                            );
                            assert_eq!(
                                byte(value, "translator-port"),
                                expected.translator,
                                "call {call}"
                            );
                            assert_eq!(byte(value, "hub-address"), expected.translator_hub);
                            let request = value.record_field("request").unwrap().unwrap();
                            assert_eq!(
                                request
                                    .record_field("setup")
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u64")
                                    .unwrap(),
                                expected.setup,
                                "call {call}"
                            );
                            let request =
                                ControlTransferRequest::new(expected.setup, &[], 256).unwrap();
                            response.encoded.clear();
                            response.encoded.extend_from_slice(
                                encoder
                                    .completed(
                                        &request,
                                        expected.reply.len() as u16,
                                        &expected.reply,
                                    )
                                    .unwrap(),
                            );
                            run.kernel_mut()
                                .admit_input(&received.port_id, call as u64, &response)
                                .unwrap();
                            call += 1;
                        }
                        "configuration-decode" => configuration.submit(&output[index]).unwrap(),
                        "descriptor-decode" => descriptor.submit(&output[index]).unwrap(),
                        "wait" => {
                            assert_eq!(
                                output[index].encoded.as_slice(),
                                delays.get(wait).expect("finite wait count").to_le_bytes()
                            );
                            run.kernel_mut()
                                .admit_input(&woke.port_id, wait as u64, &pulse)
                                .unwrap();
                            wait += 1;
                        }
                        "result" => {
                            let value = validate_canonical_structured_value(&output[index].encoded)
                                .unwrap();
                            let ready = value
                                .variant_payload("input")
                                .unwrap()
                                .expect("supported contact profile");
                            assert_eq!(byte(ready, "address"), expected_address);
                            assert_eq!(byte(ready, "hub-address"), expected_hub);
                            assert_eq!(byte(ready, "translator-port"), 2);
                            assert_eq!(byte(ready, "endpoint"), 3);
                            let layout = ready.record_field("layout").unwrap().unwrap();
                            assert_eq!(
                                layout
                                    .record_field("maximum-x")
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u16")
                                    .unwrap(),
                                1024_u16.to_le_bytes()
                            );
                            assert_eq!(
                                layout
                                    .record_field("maximum-y")
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u16")
                                    .unwrap(),
                                600_u16.to_le_bytes()
                            );
                            assert!(!result);
                            result = true;
                            run.kernel_mut().close_input(&received.port_id).unwrap();
                            run.kernel_mut().close_input(&woke.port_id).unwrap();
                            run.kernel_mut().close_input(&topology.port_id).unwrap();
                            run.kernel_mut().close_input(&layout_input.port_id).unwrap();
                            configuration.close().unwrap();
                            descriptor.close().unwrap();
                        }
                        _ => panic!("unexpected public Fore"),
                    }
                    run.kernel_mut()
                        .complete_output(&port.port_id, sequence)
                        .unwrap();
                }
            }
            if status == conduit_composite::KernelCompositeStatus::Complete
                && configuration.complete()
                && descriptor.complete()
            {
                complete = true;
                break;
            }
        }
    });
    assert_eq!(call, fixture.len());
    assert_eq!(wait, delays.len());
    assert!(
        result && complete,
        "one supported endpoint and normal closure"
    );
    assert_eq!(allocations, 0);
}
