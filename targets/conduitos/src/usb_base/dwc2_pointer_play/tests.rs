//! One named report fixture executes native admission and normal mouse plots.
use super::*;
use crate::arch::dwc2::{EndpointRoute, TransactionTranslator, TransferType, test_io};
fn native() -> (UsbNativeIdentity, NativeProtocolIssuer) {
    let identity = UsbNativeIdentity {
        host_id: "fixture/pointer-play-host".into(),
        boot_id: "fixture/pointer-play-boot".into(),
        base_id: "fixture/controller".into(),
        provider_instance_id: "fixture/owned-bus".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pio".into(),
        resource_generation_id: ResourceGenerationId("fixture/pio-generation".into()),
        envelope_id: "fixture/bus-window".into(),
        artifact_id: "fixture/compiled-control".into(),
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: "fixture/bus-authority".into(),
            contract_id: ENDPOINT_READ_AUTHORITY.into(),
            host_call_contract_id: ENDPOINT_READ_CALL.into(),
            subject_kind: ENDPOINT_READ_KIND.into(),
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            capability_id: ENDPOINT_READ_IMPLEMENTATION.into(),
        },
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        operation_contract_id: ENDPOINT_READ_CALL.into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 4,
    };
    let issuer = NativeProtocolIssuer::from_test_authority(authority, [8; 32]).unwrap();
    (identity, issuer)
}
fn completion(bytes: Option<&[u8]>) -> (u32, Vec<u32>, Vec<u32>) {
    let Some(bytes) = bytes else {
        return (3, vec![], vec![]);
    };
    let words = bytes
        .chunks(4)
        .map(|b| {
            let mut a = [0; 4];
            a[..b.len()].copy_from_slice(b);
            u32::from_le_bytes(a)
        })
        .collect();
    (3, vec![(2 << 17) | ((bytes.len() as u32) << 4)], words)
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
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
fn record(
    ty: StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    let fields = fields
        .iter()
        .map(|f| StructuredFieldValue::new(f.name(), value(f.name(), f.value_type())).unwrap())
        .collect();
    StructuredInfoValue::record(ty, fields).unwrap()
}
#[test]
fn dwc2_pointer_play_initializes_existing_mouse_history_and_delivers_one_checked_absolute_sample() {
    let (identity, issuer) = native();
    let mut controller = test_io::prepared();
    let mut report = [0_u8; 44];
    report[0] = 1;
    report[1] = 1;
    report[3..5].copy_from_slice(&1024_u16.to_le_bytes());
    report[5..7].copy_from_slice(&600_u16.to_le_bytes());
    test_io::registers(&mut controller)
        .script
        .push_back(completion(Some(&report)));
    test_io::registers(&mut controller).writes.reserve(512);
    let route = RootEndpointRoute {
        slot: 1,
        generation: 1,
        endpoint_generation: 1,
        endpoint: EndpointRoute {
            address: 2,
            endpoint: 1,
            maximum_packet: 64,
            transfer_type: TransferType::Interrupt,
            translator: None,
        },
    };
    let (mut run, schemas) =
        unsafe { Dwc2PointerPlay::prepare(controller, route, &identity, issuer) }.unwrap();
    let layout_type = field_type(&schemas.initialize, "layout");
    let layout = fixture_layout(layout_type);
    let parameters = record(schemas.initialize.clone(), |name, ty| match name {
        "layout" => layout.clone(),
        "length" => StructuredInfoValue::leaf(ty.clone(), 64_u64.to_le_bytes().to_vec()).unwrap(),
        "queue-capacity" => {
            StructuredInfoValue::leaf(ty.clone(), 1_u64.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("field"),
    });
    let boundary = run.kernel_mut().definition().boundary.clone();
    let input = |name: &str| {
        boundary
            .input_fronts
            .iter()
            .find(|p| p.external_port.port_id.as_str() == name)
            .unwrap()
            .external_port
            .clone()
    };
    let initialize = input("initialize");
    let read = input("read");
    let parameters = ValuePayload {
        value_kind: initialize.value_kind,
        encoded: parameters.canonical_bytes().unwrap(),
    };
    let mut context = ValuePayload {
        value_kind: read.value_kind,
        encoded: Vec::with_capacity(4096),
    };
    let mut outputs: Vec<_> = boundary
        .output_fronts
        .iter()
        .map(|p| {
            (
                p.external_port.port_id.clone(),
                ValuePayload {
                    value_kind: p.external_port.value_kind.clone(),
                    encoded: Vec::with_capacity(4096),
                },
            )
        })
        .collect();
    let mut samples = 0;
    let mut contexts = 0;
    let mut received = 0;
    let mut decoded = 0;
    let mut complete = false;
    let allocations = crate::test_allocations::allocations(|| {
        run.kernel_mut().start().unwrap();
        assert!(matches!(
            run.kernel_mut()
                .admit_input(&initialize.port_id, 0, &parameters)
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ));
        for _ in 0..2048 {
            let status = run.step().unwrap();
            assert_ne!(status, KernelCompositeStatus::Cancelled);
            for (port, value) in &mut outputs {
                if let Some(sequence) = run.kernel_mut().output_into(port, value).unwrap() {
                    if port.as_str() == "context" {
                        contexts += 1;
                        run.kernel_mut().close_input(&initialize.port_id).unwrap();
                        context.encoded.clear();
                        context.encoded.extend_from_slice(&value.encoded);
                        assert!(matches!(
                            run.kernel_mut()
                                .admit_input(&read.port_id, 0, &context)
                                .unwrap(),
                            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
                        ));
                    }
                    if port.as_str() == "received" || port.as_str() == "decoded" {
                        let value = validate_canonical_structured_value(&value.encoded).unwrap();
                        let tags = if port.as_str() == "received" {
                            &[
                                "report",
                                "idle",
                                "malformed",
                                "stalled",
                                "provider-lost",
                                "unsupported",
                            ][..]
                        } else {
                            &[
                                "point",
                                "short",
                                "malformed",
                                "other-report",
                                "invalid-layout",
                            ][..]
                        };
                        for (index, tag) in tags.iter().enumerate() {
                            if value.variant_payload(tag).unwrap().is_some() {
                                if port.as_str() == "received" {
                                    received = index + 1
                                } else {
                                    decoded = index + 1
                                }
                            }
                        }
                    }
                    if port.as_str() == "update" {
                        let value = validate_canonical_structured_value(&value.encoded).unwrap();
                        let ready = value.variant_payload("ready").unwrap().unwrap();
                        let sample = ready.record_field("sample").unwrap().unwrap();
                        for name in ["position-x", "position-y"] {
                            assert_eq!(
                                sample
                                    .record_field(name)
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/i64")
                                    .unwrap(),
                                1_000_000_i64.to_le_bytes()
                            );
                        }
                        assert_eq!(
                            sample
                                .record_field("sequence")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/u64")
                                .unwrap(),
                            1_u64.to_le_bytes()
                        );
                        assert_eq!(
                            sample
                                .record_field("primary-pressed")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/bool")
                                .unwrap(),
                            [1]
                        );
                        samples += 1;
                        run.kernel_mut().close_input(&read.port_id).unwrap();
                    }
                    run.kernel_mut().complete_output(port, sequence).unwrap();
                }
            }
            if status == KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
    });
    if samples == 0 {
        eprintln!("signs={:?}", run.kernel_mut().signs());
        for (i, g) in run.kernel_mut().definition().internal_plan.fragments[0]
            .placements
            .iter()
            .enumerate()
        {
            eprintln!("node {i} {}", g.gear_id.as_str());
        }
    }
    assert_eq!(
        samples, 1,
        "contexts={contexts} received={received} decoded={decoded} complete={complete}"
    );
    assert!(complete);
    assert_eq!(allocations, 0);
}

fn fixture_layout(layout_type: StructuredInfoType) -> StructuredInfoValue {
    record(layout_type, |name, ty| {
        let bytes = match name {
            "report-id" => vec![1],
            "tip-byte" => 1_u64.to_le_bytes().to_vec(),
            "tip-mask" => vec![1],
            "x-byte" => 3_u64.to_le_bytes().to_vec(),
            "y-byte" => 5_u64.to_le_bytes().to_vec(),
            "x-width" | "y-width" => vec![2],
            "maximum-x" => 1024_u16.to_le_bytes().to_vec(),
            "maximum-y" => 600_u16.to_le_bytes().to_vec(),
            _ => panic!("field"),
        };
        StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
    })
}

#[test]
fn dwc2_pointer_session_carries_source_history_through_press_and_release_without_allocating() {
    let (identity, issuer) = native();
    let mut controller = test_io::prepared();
    let io = test_io::registers(&mut controller);
    io.clock_step = 10;
    let split_ack = || (0x22, vec![], vec![]);
    io.script.extend([split_ack(), (0x12, vec![], vec![])]);
    for (pressed, x, y) in [(true, 1024_u16, 600_u16), (false, 512, 300)] {
        let mut report = [0_u8; 44];
        report[0] = 1;
        report[1] = u8::from(pressed);
        report[3..5].copy_from_slice(&x.to_le_bytes());
        report[5..7].copy_from_slice(&y.to_le_bytes());
        io.script.extend([split_ack(), completion(Some(&report))]);
    }
    io.writes.reserve(1024);
    let route = RootEndpointRoute {
        slot: 1,
        generation: 1,
        endpoint_generation: 1,
        endpoint: EndpointRoute {
            address: 2,
            endpoint: 1,
            maximum_packet: 64,
            transfer_type: TransferType::Interrupt,
            translator: Some(TransactionTranslator {
                hub_address: 1,
                port: 2,
                low_speed: false,
            }),
        },
    };
    let (play, schemas) =
        unsafe { Dwc2PointerPlay::prepare(controller, route, &identity, issuer) }.unwrap();
    let layout_type = field_type(&schemas.initialize, "layout");
    let layout = fixture_layout(layout_type.clone());
    let length_type = field_type(&schemas.initialize, "length");
    let selected_type = StructuredInfoType::record(
        kind_id("fixture/selected-pointer"),
        vec![
            StructuredFieldType::new("layout", layout_type).unwrap(),
            StructuredFieldType::new("maximum-packet", length_type).unwrap(),
        ],
    )
    .unwrap();
    let selected = record(selected_type, |name, ty| {
        if name == "layout" {
            layout.clone()
        } else {
            StructuredInfoValue::leaf(ty.clone(), 64_u64.to_le_bytes().to_vec()).unwrap()
        }
    })
    .canonical_bytes()
    .unwrap();
    let selected = validate_canonical_structured_value(&selected).unwrap();
    let mut session =
        super::super::dwc2_pointer_session::Dwc2PointerSession::prepare(play, schemas, selected)
            .unwrap();
    let mut reads = 0;
    let mut samples = 0;
    let allocations = crate::test_allocations::allocations(|| {
        session.start().unwrap();
        for _ in 0..4096 {
            if session.ready() && reads < 3 {
                session.read().unwrap();
                reads += 1;
            }
            if let Some(sample) = session.step().unwrap() {
                samples += 1;
                assert_eq!(sample.sequence, samples);
                assert_eq!(sample.primary_pressed, samples == 1);
                assert_eq!(
                    (sample.position_x, sample.position_y),
                    if samples == 1 {
                        (1_000_000, 1_000_000)
                    } else {
                        (500_000, 500_000)
                    }
                );
                if samples == 2 {
                    break;
                }
            }
        }
    });
    assert_eq!((reads, samples), (3, 2));
    assert_eq!(allocations, 0);
}
