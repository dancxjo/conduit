//! Checked ECM class policy, below native USB/network authority and physics.
use conduit_core::*;
use conduit_plot::PortableExpressionProgram;
use conduitos::protocol_source::*;

fn source() -> PreparedProtocolSource {
    let service = include_str!("../plots/usb/cdc-ecm-service.conduit");
    let receive = include_str!("../plots/usb/cdc-ecm-frames.conduit");
    let mut imports = Vec::new();
    for line in service.lines().chain(receive.lines()) {
        if line.starts_with("with ") && !imports.contains(&line) {
            imports.push(line);
        }
    }
    let body = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    PreparedProtocolSource::prepare(
        ProtocolSourcePackage::compile(
            format!(
                "{}\n{}\n{}\n{}\n{}",
                imports.join("\n"),
                include_str!("../plots/usb/cdc-ecm-types.conduit"),
                include_str!("../plots/usb/cdc-ecm-profile.conduit"),
                body(receive),
                body(service)
            ),
            &[
                ProtocolSpecializationRequest::SeededUntil {
                    value: value("CdcEcmState"),
                },
                ProtocolSpecializationRequest::FeedbackZip {
                    left: value("CdcEcmState"),
                    right: value("CdcEcmEvent"),
                },
                ProtocolSpecializationRequest::Merge {
                    value: value("CdcEcmEvent"),
                },
            ],
        )
        .unwrap(),
    )
    .unwrap()
}
fn program(source: &PreparedProtocolSource, name: &str) -> PortableExpressionProgram {
    let expanded = source
        .expand(name)
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn eval(p: &PortableExpressionProgram, v: &StructuredInfoValue) -> StructuredInfoValue {
    let bytes = p.evaluate(&v.canonical_bytes().unwrap()).unwrap();
    if matches!(p.output_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
        StructuredInfoValue::leaf(p.output_type.clone(), bytes).unwrap()
    } else {
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    }
}
fn fields(ty: &StructuredInfoType) -> &[StructuredFieldType] {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    fields(ty)
        .iter()
        .find(|v| v.name() == name)
        .unwrap()
        .value_type()
}
fn case<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|v| v.tag() == tag)
        .unwrap()
        .payload_type()
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
}
fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn variant(ty: &StructuredInfoType, name: &str, value: StructuredInfoValue) -> StructuredInfoValue {
    StructuredInfoValue::variant(ty.clone(), name, value).unwrap()
}
fn unit(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    variant(ty, tag, leaf(case(ty, tag), &[]))
}
fn member<'a>(v: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fs) = v.shape() else {
        panic!("record")
    };
    fs.iter().find(|f| f.name() == name).unwrap().value()
}
fn number(v: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = v.shape() else {
        panic!("leaf")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn tag(v: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = v.shape() else {
        panic!("variant")
    };
    tag
}
fn start(s: &PreparedProtocolSource, terminator: &[u8]) -> StructuredInfoValue {
    let p = program(s, "cdc-ecm-start");
    let ty = field(&p.input_type, "terminator");
    let input = record(
        &p.input_type,
        vec![(
            "terminator",
            record(ty, vec![("wire", leaf(field(ty, "wire"), terminator))]),
        )],
    );
    eval(&p, &input)
}
fn change(
    state: &StructuredInfoValue,
    name: &str,
    value: StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fs) = state.shape() else {
        panic!("record")
    };
    record(
        state.value_type(),
        fs.iter()
            .map(|f| {
                (
                    f.name(),
                    if f.name() == name {
                        value.clone()
                    } else {
                        f.value().clone()
                    },
                )
            })
            .collect(),
    )
}
fn phase(state: &StructuredInfoValue, n: u64) -> StructuredInfoValue {
    change(
        state,
        "phase",
        leaf(field(state.value_type(), "phase"), &n.to_le_bytes()),
    )
}
fn step(
    s: &PreparedProtocolSource,
    state: &StructuredInfoValue,
    event: &StructuredInfoValue,
) -> StructuredInfoValue {
    let accept = program(s, "cdc-ecm-accept");
    let fs = fields(&accept.input_type);
    let pair = record(
        &accept.input_type,
        vec![(fs[0].name(), state.clone()), (fs[1].name(), event.clone())],
    );
    eval(&program(s, "cdc-ecm-advance"), &eval(&accept, &pair))
}
fn frame(length: usize) -> Vec<u8> {
    let mut wire = vec![0x55; length];
    wire[..6].fill(0xff);
    wire[6..12].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
    wire[12..14].copy_from_slice(&[8, 0]);
    wire
}
fn transmit(s: &PreparedProtocolSource, ordinal: u64, wire: &[u8]) -> StructuredInfoValue {
    let p = program(s, "cdc-ecm-frame-event");
    let ty = case(&p.input_type, "transmit");
    eval(
        &p,
        &variant(
            &p.input_type,
            "transmit",
            record(
                ty,
                vec![
                    (
                        "ordinal",
                        leaf(field(ty, "ordinal"), &ordinal.to_le_bytes()),
                    ),
                    ("wire", leaf(field(ty, "wire"), wire)),
                ],
            ),
        ),
    )
}
fn written(s: &PreparedProtocolSource, actual: u64) -> StructuredInfoValue {
    let p = program(s, "cdc-ecm-write-event");
    let ty = case(&p.input_type, "completed");
    eval(
        &p,
        &variant(
            &p.input_type,
            "completed",
            record(
                ty,
                vec![("actual", leaf(field(ty, "actual"), &actual.to_le_bytes()))],
            ),
        ),
    )
}
fn queue(s: &PreparedProtocolSource, name: &str) -> StructuredInfoValue {
    let p = program(s, "cdc-ecm-frame-event");
    eval(&p, &unit(&p.input_type, name))
}
#[test]
fn entry_has_one_independent_queue_and_three_transport_placements() {
    let s = source();
    let expanded = s.expand("cdc-ecm-frame-service").unwrap();
    for kind in [
        "network/ethernet-frame",
        "machine/usb/control",
        "machine/usb/endpoint-read",
        "machine/usb/endpoint-write",
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == kind)
                .count(),
            1
        );
    }
}
#[test]
fn five_setup_requests_are_exact_and_empty_terminator_is_checked_before_effects() {
    let s = source();
    let p = program(&s, "cdc-ecm-control-request");
    let state = start(&s, &[]);
    for (i, setup) in [
        0x0050000002000680u64,
        0x0000000000010900,
        0x001a040903040680,
        0x00000000000f4321,
        0x0000000100010b01,
    ]
    .iter()
    .enumerate()
    {
        let value = eval(&p, &phase(&state, i as u64 + 1));
        assert_eq!(number(member(&value, "setup")), *setup);
    }
    assert_eq!(
        tag(&eval(&program(&s, "cdc-ecm-route"), &start(&s, &[1]))),
        "stopped"
    );
}
#[test]
fn aligned_transmit_needs_a_real_zlp_before_exact_queue_ack() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 6);
    state = step(&s, &state, &transmit(&s, 0, &frame(64)));
    assert_eq!(number(member(&state, "phase")), 7);
    state = step(&s, &state, &written(&s, 64));
    assert_eq!(number(member(&state, "phase")), 8);
    let req = eval(&program(&s, "cdc-ecm-write-request"), &state);
    let StructuredInfoValueShape::Leaf(wire) = member(&req, "wire").shape() else {
        panic!()
    };
    assert!(wire.is_empty());
    state = step(&s, &state, &written(&s, 0));
    assert_eq!(number(member(&state, "phase")), 9);
    let req = eval(&program(&s, "cdc-ecm-queue-request"), &state);
    assert_eq!(tag(&req), "acknowledge-transmit");
    state = step(&s, &state, &queue(&s, "acknowledged"));
    assert_eq!(number(member(&state, "phase")), 10);
}
#[test]
fn short_write_stale_ordinal_and_queue_faults_terminate_without_retry_or_ack() {
    let s = source();
    let state = phase(&start(&s, &[]), 6);
    let stale = step(&s, &state, &transmit(&s, 1, &frame(14)));
    assert_eq!(tag(member(&stale, "outcome")), "malformed");
    let written_state = step(&s, &state, &transmit(&s, 0, &frame(14)));
    let short = step(&s, &written_state, &written(&s, 13));
    assert_eq!(tag(member(&short, "outcome")), "malformed");
    for fault in [
        "pressure",
        "stale",
        "cancelled",
        "sequence-exhausted",
        "provider-lost",
        "malformed",
    ] {
        let stopped = step(&s, &state, &queue(&s, fault));
        assert_eq!(tag(member(&stopped, "outcome")), fault);
        assert_eq!(
            tag(&eval(&program(&s, "cdc-ecm-route"), &stopped)),
            "stopped"
        );
    }
}
#[test]
fn sixteenth_delivery_is_terminal_and_no_seventeenth_usb_or_queue_effect_is_routed() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 11);
    state = change(
        &state,
        "cycles",
        leaf(field(state.value_type(), "cycles"), &15u64.to_le_bytes()),
    );
    let stopped = step(&s, &state, &queue(&s, "accepted"));
    assert_eq!(number(member(&stopped, "cycles")), 16);
    assert_eq!(tag(member(&stopped, "outcome")), "completed");
    assert_eq!(
        tag(&eval(&program(&s, "cdc-ecm-route"), &stopped)),
        "stopped"
    );
}
fn control_result(
    ty: &StructuredInfoType,
    wire: &[u8],
    actual: u64,
    short: bool,
) -> StructuredInfoValue {
    let inner = case(ty, "completed");
    let input = field(inner, "input");
    let StructuredInfoTypeShape::Sequence { element, .. } = input.shape() else {
        panic!("sequence")
    };
    variant(
        ty,
        "completed",
        record(
            inner,
            vec![
                (
                    "transferred",
                    leaf(
                        field(inner, "transferred"),
                        &u16::try_from(actual).unwrap().to_le_bytes(),
                    ),
                ),
                ("short", leaf(field(inner, "short"), &[u8::from(short)])),
                (
                    "input",
                    StructuredInfoValue::sequence(
                        input.clone(),
                        wire.iter().map(|b| leaf(element, &[*b])).collect(),
                    )
                    .unwrap(),
                ),
            ],
        ),
    )
}
#[test]
fn exact_descriptor_mac_and_ack_flags_refuse_every_changed_byte_and_truncation() {
    let s = source();
    let config = [
        9, 2, 80, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 6, 0, 0, 5, 36, 0, 16, 1, 5, 36, 6, 0, 1,
        13, 36, 15, 4, 0, 0, 0, 0, 234, 5, 0, 0, 0, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 0, 10, 0,
        0, 0, 9, 4, 1, 1, 2, 10, 0, 0, 0, 7, 5, 129, 2, 64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
    ];
    let mut mac = vec![26, 3];
    for b in b"020000000001" {
        mac.extend([*b, 0]);
    }
    for (name, wire) in [
        ("cdc-ecm-topology-supported", config.as_slice()),
        ("cdc-ecm-mac-supported", mac.as_slice()),
    ] {
        let p = program(&s, name);
        let supported = eval(
            &p,
            &control_result(&p.input_type, wire, wire.len() as u64, false),
        );
        assert_eq!(supported, leaf(&p.output_type, &[1]));
        for index in 0..wire.len() {
            let mut wrong = wire.to_vec();
            wrong[index] ^= 1;
            assert_eq!(
                eval(
                    &p,
                    &control_result(&p.input_type, &wrong, wire.len() as u64, false)
                ),
                leaf(&p.output_type, &[0])
            );
        }
        assert_eq!(
            eval(
                &p,
                &control_result(
                    &p.input_type,
                    &wire[..wire.len() - 1],
                    wire.len() as u64,
                    false
                )
            ),
            leaf(&p.output_type, &[0])
        );
        assert_eq!(
            eval(
                &p,
                &control_result(&p.input_type, wire, wire.len() as u64, true)
            ),
            leaf(&p.output_type, &[0])
        );
    }
    let p = program(&s, "cdc-ecm-no-data-ack");
    assert_eq!(
        eval(&p, &control_result(&p.input_type, &[], 0, false)),
        leaf(&p.output_type, &[1])
    );
    assert_eq!(
        eval(&p, &control_result(&p.input_type, &[], 0, true)),
        leaf(&p.output_type, &[0])
    );
}
#[path = "usb_ecm_class_source/reuse.rs"]
mod reuse;
