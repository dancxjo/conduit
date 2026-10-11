//! Ordinary Source checks/expansion only; no emulated or physical CDC proof.
use conduitos::protocol_source::{
    PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};

fn value(name: &str) -> ProtocolValueReference {
    ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    }
}
fn package() -> ProtocolSourcePackage {
    let lifecycle = include_str!("../plots/usb/cdc-acm-lifecycle.conduit");
    let (header, body) = lifecycle.split_once("\n\n").unwrap();
    ProtocolSourcePackage::compile(
        format!(
            "{header}\n{}\n{}\n{body}",
            include_str!("../plots/usb/cdc-acm-types.conduit"),
            include_str!("../plots/usb/cdc-acm-profile.conduit")
        ),
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: value("CdcAcmState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("CdcAcmState"),
                right: value("CdcAcmEvent"),
            },
            ProtocolSpecializationRequest::Zip {
                left: value("UsbControlResult"),
                right: ProtocolValueReference {
                    type_name: "CdcAcmDescriptorSupported".into(),
                    maximum_bytes: 256,
                },
            },
            ProtocolSpecializationRequest::Merge {
                value: value("CdcAcmEvent"),
            },
        ],
    )
    .unwrap()
}

#[test]
fn ordinary_cdc_source_checks_and_reuses_one_placement_per_transport_operation() {
    let prepared = PreparedProtocolSource::prepare(package()).unwrap();
    let expanded = prepared.expand("cdc-acm-serial").unwrap();
    for kind in [
        "machine/usb/control",
        "machine/usb/endpoint-read",
        "machine/usb/endpoint-write",
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == kind)
                .count(),
            1,
            "{kind}"
        );
    }
}

#[test]
fn pure_first_profile_and_lifecycle_functions_expand_independently() {
    let prepared = PreparedProtocolSource::prepare(package()).unwrap();
    for entry in [
        "cdc-acm-topology-supported",
        "cdc-acm-no-data-ack",
        "cdc-acm-line-coding-ack",
        "cdc-acm-control-request",
        "cdc-acm-descriptor-flag",
        "cdc-acm-control-event",
        "cdc-acm-received-wire",
        "cdc-acm-write-request",
        "cdc-acm-read-request",
        "cdc-acm-initialize",
        "cdc-acm-route",
        "cdc-acm-accept",
        "cdc-acm-advance",
        "cdc-acm-received-bytes",
    ] {
        assert_eq!(
            prepared.expand(entry).unwrap().expanded.gears.len(),
            1,
            "{entry}"
        );
    }
}

fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    use conduit_core::ConfigurationValue;
    let prepared = PreparedProtocolSource::prepare(package()).unwrap();
    let expanded = prepared.expand(entry).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn typed_transmit_preserves_wire_and_gates_empty_or_over_64_before_class_effects() {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
        StructuredInfoValueShape,
    };
    let initialize = program("cdc-acm-initialize");
    let route = program("cdc-acm-route");
    let StructuredInfoTypeShape::Record { fields, .. } = initialize.input_type.shape() else {
        panic!()
    };
    let wire_type = fields
        .iter()
        .find(|field| field.name() == "wire")
        .unwrap()
        .value_type();
    for length in [0, 1, 64, 65, 2048] {
        let wire = vec![0x41; length];
        let request = StructuredInfoValue::record(
            initialize.input_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "wire",
                    StructuredInfoValue::leaf(wire_type.clone(), wire).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let initialized = initialize
            .evaluate(&request.canonical_bytes().unwrap())
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&initialized).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!()
        };
        assert_eq!(
            fields
                .iter()
                .find(|field| field.name() == "transmit")
                .unwrap()
                .value(),
            &request
        );
        let routed =
            StructuredInfoValue::from_canonical_bytes(&route.evaluate(&initialized).unwrap())
                .unwrap();
        let StructuredInfoValueShape::Variant { tag, .. } = routed.shape() else {
            panic!()
        };
        assert_eq!(
            tag,
            if length == 0 || length > 64 {
                "stopped"
            } else {
                "control"
            }
        );
    }
}

#[test]
fn exact_descriptor_profile_accepts_only_all_67_reviewed_octets() {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    };
    fn completion(ty: &StructuredInfoType, descriptor: &[u8]) -> Vec<u8> {
        let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
            panic!()
        };
        let payload_type = cases
            .iter()
            .find(|case| case.tag() == "completed")
            .unwrap()
            .payload_type();
        let StructuredInfoTypeShape::Record { fields, .. } = payload_type.shape() else {
            panic!()
        };
        let payload = StructuredInfoValue::record(
            payload_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = match field.name() {
                        "transferred" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            (descriptor.len() as u16).to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "short" => {
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap()
                        }
                        "input" => {
                            let StructuredInfoTypeShape::Sequence { element, .. } =
                                field.value_type().shape()
                            else {
                                panic!()
                            };
                            StructuredInfoValue::sequence(
                                field.value_type().clone(),
                                descriptor
                                    .iter()
                                    .map(|byte| {
                                        StructuredInfoValue::leaf(element.clone(), vec![*byte])
                                            .unwrap()
                                    })
                                    .collect(),
                            )
                            .unwrap()
                        }
                        _ => panic!(),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap();
        StructuredInfoValue::variant(ty.clone(), "completed", payload)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }
    let profile = program("cdc-acm-topology-supported");
    let descriptor = [
        9, 2, 67, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 2, 1, 0, 5, 36, 0, 16, 1, 5, 36, 1, 0, 1,
        4, 36, 2, 2, 5, 36, 6, 0, 1, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 2, 10, 0, 0, 0, 7, 5,
        129, 2, 64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
    ];
    assert_eq!(
        profile
            .evaluate(&completion(&profile.input_type, &descriptor))
            .unwrap(),
        [1]
    );
    for index in 0..descriptor.len() {
        let mut foreign = descriptor;
        foreign[index] ^= 1;
        assert_eq!(
            profile
                .evaluate(&completion(&profile.input_type, &foreign))
                .unwrap(),
            [0],
            "octet {index}"
        );
    }
    assert_eq!(
        profile
            .evaluate(&completion(&profile.input_type, &descriptor[..66]))
            .unwrap(),
        [0]
    );
}
