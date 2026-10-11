//! Checked BOT Source conformance. No hardware, kernel execution or class proof.
use conduit_core::*;
use conduit_plot::PortableExpressionProgram;
use conduitos::protocol_source::*;

const CBW: [u8; 31] = [
    85, 83, 66, 67, 1, 0, 0, 0, 0, 2, 0, 0, 128, 0, 10, 40, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
    0, 0,
];
const CSW: [u8; 13] = [85, 83, 66, 83, 1, 0, 0, 0, 0, 0, 0, 0, 0];
const CONFIG: [u8; 32] = [
    9, 2, 32, 0, 1, 1, 5, 192, 0, 9, 4, 0, 0, 2, 8, 6, 80, 0, 7, 5, 129, 2, 0, 2, 0, 7, 5, 2, 2, 0,
    2, 0,
];

fn prepared() -> PreparedProtocolSource {
    let lifecycle = include_str!("../plots/usb/bot-lifecycle.conduit");
    let (header, body) = lifecycle.split_once("\n\n").unwrap();
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        format!(
            "{header}\n{}\n{}\n{body}",
            include_str!("../plots/usb/bot-types.conduit"),
            include_str!("../plots/usb/bot-profile.conduit")
        ),
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: value("BotState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("BotState"),
                right: value("BotEvent"),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("BotEvent"),
            },
        ],
    )
    .unwrap();
    PreparedProtocolSource::prepare(package).unwrap()
}
fn program(source: &PreparedProtocolSource, name: &str) -> PortableExpressionProgram {
    let expanded = source.expand(name).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1, "{name}");
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn fields(ty: &StructuredInfoType) -> &[StructuredFieldType] {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    fields(ty)
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn payload_type<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
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
        fields(ty)
            .iter()
            .map(|field| {
                StructuredFieldValue::new(
                    field.name(),
                    values
                        .iter()
                        .find(|(name, _)| *name == field.name())
                        .unwrap()
                        .1
                        .clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn write_request(ty: &StructuredInfoType, wire: &[u8]) -> StructuredInfoValue {
    record(ty, vec![("wire", leaf(field_type(ty, "wire"), wire))])
}
fn read_result(
    ty: &StructuredInfoType,
    ordinal: u64,
    actual: u64,
    short: bool,
    wire: &[u8],
) -> StructuredInfoValue {
    let frame = payload_type(ty, "completed");
    let value = record(
        frame,
        vec![
            (
                "ordinal",
                leaf(field_type(frame, "ordinal"), &ordinal.to_le_bytes()),
            ),
            (
                "actual",
                leaf(field_type(frame, "actual"), &actual.to_le_bytes()),
            ),
            (
                "short",
                leaf(field_type(frame, "short"), &[u8::from(short)]),
            ),
            ("wire", leaf(field_type(frame, "wire"), wire)),
        ],
    );
    StructuredInfoValue::variant(ty.clone(), "completed", value).unwrap()
}
fn control_result(ty: &StructuredInfoType, input: &[u8]) -> StructuredInfoValue {
    let frame = payload_type(ty, "completed");
    let input_type = field_type(frame, "input");
    let StructuredInfoTypeShape::Sequence { element, .. } = input_type.shape() else {
        panic!()
    };
    let value = record(
        frame,
        vec![
            (
                "transferred",
                leaf(
                    field_type(frame, "transferred"),
                    &(input.len() as u16).to_le_bytes(),
                ),
            ),
            ("short", leaf(field_type(frame, "short"), &[0])),
            (
                "input",
                StructuredInfoValue::sequence(
                    input_type.clone(),
                    input.iter().map(|byte| leaf(element, &[*byte])).collect(),
                )
                .unwrap(),
            ),
        ],
    );
    StructuredInfoValue::variant(ty.clone(), "completed", value).unwrap()
}
fn write_result(ty: &StructuredInfoType, actual: u64) -> StructuredInfoValue {
    let frame = payload_type(ty, "completed");
    StructuredInfoValue::variant(
        ty.clone(),
        "completed",
        record(
            frame,
            vec![(
                "actual",
                leaf(field_type(frame, "actual"), &actual.to_le_bytes()),
            )],
        ),
    )
    .unwrap()
}
fn terminal(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    StructuredInfoValue::variant(ty.clone(), tag, leaf(payload_type(ty, tag), &[])).unwrap()
}
fn evaluate(
    program: &PortableExpressionProgram,
    value: &StructuredInfoValue,
) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(
        &program.evaluate(&value.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap()
}
fn member<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn scalar(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!()
    };
    bytes
}
fn phase(state: &StructuredInfoValue) -> u64 {
    u64::from_le_bytes(scalar(member(state, "phase")).try_into().unwrap())
}
fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!()
    };
    tag
}

#[test]
fn bounded_bot_checks_with_one_transport_placement_and_two_distinct_feedback_retention_owners() {
    let source = prepared();
    let expanded = source.expand("bot-read-block").unwrap();
    for kind in [
        "machine/usb/control",
        "machine/usb/endpoint-write",
        "machine/usb/endpoint-read",
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == kind)
                .count(),
            1
        );
    }
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "flow/zip/feedback")
            .count(),
        2
    );
    assert!(!expanded
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == "flow/zip/finite"));
}

#[test]
fn every_cbw_octet_and_length_is_checked_before_a_class_effect_and_command_is_preserved() {
    let source = prepared();
    let check = program(&source, "bot-cbw-supported");
    let initialize = program(&source, "bot-initialize");
    let route = program(&source, "bot-route");
    let valid = write_request(&initialize.input_type, &CBW);
    assert_eq!(
        check.evaluate(&valid.canonical_bytes().unwrap()).unwrap(),
        [1]
    );
    let state = evaluate(&initialize, &valid);
    assert_eq!(member(&state, "command"), &valid);
    assert_eq!(tag(&evaluate(&route, &state)), "control");
    let mut cases = Vec::new();
    for index in 0..31 {
        let mut foreign = CBW.to_vec();
        foreign[index] ^= 1;
        cases.push(foreign);
    }
    cases.extend([
        vec![],
        CBW[..30].to_vec(),
        [CBW.as_slice(), &[0]].concat(),
        vec![0; 64],
    ]);
    for wire in cases {
        let request = write_request(&initialize.input_type, &wire);
        assert_eq!(
            check.evaluate(&request.canonical_bytes().unwrap()).unwrap(),
            [0]
        );
        let state = evaluate(&initialize, &request);
        assert_eq!(scalar(member(&state, "refused")), [1]);
        assert_eq!(tag(&evaluate(&route, &state)), "stopped");
    }
}

#[test]
fn pinned_high_speed_topology_refuses_every_foreign_descriptor_octet() {
    let source = prepared();
    let check = program(&source, "bot-topology-supported");
    assert_eq!(
        check
            .evaluate(
                &control_result(&check.input_type, &CONFIG)
                    .canonical_bytes()
                    .unwrap()
            )
            .unwrap(),
        [1]
    );
    for index in 0..32 {
        let mut bytes = CONFIG;
        bytes[index] ^= 1;
        assert_eq!(
            check
                .evaluate(
                    &control_result(&check.input_type, &bytes)
                        .canonical_bytes()
                        .unwrap()
                )
                .unwrap(),
            [0],
            "octet{index}"
        );
    }
    assert_eq!(
        check
            .evaluate(
                &control_result(&check.input_type, &CONFIG[..31])
                    .canonical_bytes()
                    .unwrap()
            )
            .unwrap(),
        [0]
    );
}

#[test]
fn status_signature_tag_residue_status_and_metadata_must_all_match() {
    let source = prepared();
    let csw = program(&source, "bot-csw-supported");
    let read_event = program(&source, "bot-read-event");
    let data = program(&source, "bot-data-supported");
    let valid = read_result(&csw.input_type, 1, 13, false, &CSW);
    assert_eq!(
        csw.evaluate(&valid.canonical_bytes().unwrap()).unwrap(),
        [1]
    );
    for index in 0..13 {
        let mut wire = CSW;
        wire[index] ^= 1;
        let event = evaluate(
            &read_event,
            &read_result(&read_event.input_type, 1, 13, false, &wire),
        );
        let StructuredInfoValueShape::Variant { payload, .. } = event.shape() else {
            panic!()
        };
        assert_eq!(scalar(member(payload, "status")), [0]);
        assert_eq!(
            csw.evaluate(
                &read_result(&csw.input_type, 1, 13, false, &wire)
                    .canonical_bytes()
                    .unwrap()
            )
            .unwrap(),
            [0],
            "octet{index}"
        );
    }
    for (ordinal, actual, short, wire) in [
        (0, 13, false, CSW.to_vec()),
        (2, 13, false, CSW.to_vec()),
        (1, 12, false, CSW.to_vec()),
        (1, 13, true, CSW.to_vec()),
        (1, 12, false, CSW[..12].to_vec()),
        (1, 14, false, [CSW.as_slice(), &[0]].concat()),
    ] {
        assert_eq!(
            csw.evaluate(
                &read_result(&csw.input_type, ordinal, actual, short, &wire)
                    .canonical_bytes()
                    .unwrap()
            )
            .unwrap(),
            [0]
        );
    }
    assert_eq!(
        data.evaluate(
            &read_result(&data.input_type, 0, 512, false, &[0; 512])
                .canonical_bytes()
                .unwrap()
        )
        .unwrap(),
        [1]
    );
    for (ordinal, actual, short, len) in [
        (1, 512, false, 512),
        (0, 511, false, 512),
        (0, 512, true, 512),
        (0, 512, false, 511),
    ] {
        assert_eq!(
            data.evaluate(
                &read_result(&data.input_type, ordinal, actual, short, &vec![0; len])
                    .canonical_bytes()
                    .unwrap()
            )
            .unwrap(),
            [0]
        );
    }
    for name in ["stalled", "provider-lost", "unsupported", "timeout"] {
        assert_eq!(
            csw.evaluate(&terminal(&csw.input_type, name).canonical_bytes().unwrap())
                .unwrap(),
            [0]
        );
        assert_eq!(
            data.evaluate(&terminal(&data.input_type, name).canonical_bytes().unwrap())
                .unwrap(),
            [0]
        );
    }
}

fn transition(
    accept: &PortableExpressionProgram,
    advance: &PortableExpressionProgram,
    state: &StructuredInfoValue,
    event: &StructuredInfoValue,
) -> StructuredInfoValue {
    let pair = StructuredInfoValue::record(
        accept.input_type.clone(),
        fields(&accept.input_type)
            .iter()
            .zip([state, event])
            .map(|(field, value)| StructuredFieldValue::new(field.name(), value.clone()).unwrap())
            .collect(),
    )
    .unwrap();
    evaluate(advance, &evaluate(accept, &pair))
}

#[test]
fn sixteen_serial_transactions_publish_only_after_each_status_and_never_submit_seventeenth() {
    let source = prepared();
    let init = program(&source, "bot-initialize");
    let accept = program(&source, "bot-accept");
    let advance = program(&source, "bot-advance");
    let outcome = program(&source, "bot-outcome");
    let route = program(&source, "bot-route");
    let accepted = program(&source, "bot-accepted-state");
    let control = program(&source, "bot-control-event");
    let write = program(&source, "bot-write-event");
    let read = program(&source, "bot-read-event");
    let mut state = evaluate(&init, &write_request(&init.input_type, &CBW));
    for wire in [CONFIG.as_slice(), &[]] {
        let event = evaluate(&control, &control_result(&control.input_type, wire));
        state = transition(&accept, &advance, &state, &event);
        assert_eq!(tag(&evaluate(&accepted, &state)), "refused");
    }
    for transaction in 0..16u64 {
        assert_eq!(phase(&state), 2);
        assert_eq!(tag(&evaluate(&route, &state)), "write");
        // The first transaction has no previous accepted block; later states
        // expose the previous block exactly at this post-CSW boundary.
        assert_eq!(
            tag(&evaluate(&accepted, &state)),
            if transaction == 0 { "refused" } else { "ready" }
        );
        let partial = evaluate(&write, &write_result(&write.input_type, 30));
        let fault = transition(&accept, &advance, &state, &partial);
        assert_eq!(tag(&evaluate(&route, &fault)), "stopped");
        assert_eq!(tag(&evaluate(&accepted, &fault)), "refused");
        let written = evaluate(&write, &write_result(&write.input_type, 31));
        state = transition(&accept, &advance, &state, &written);
        assert_eq!(phase(&state), 3);
        assert_eq!(tag(&evaluate(&accepted, &state)), "refused");
        let data = evaluate(
            &read,
            &read_result(&read.input_type, 2 * transaction, 512, false, &[0x5a; 512]),
        );
        // Both a future result and an earlier transaction's result refuse.
        for ordinal in [
            2 * transaction + 2,
            2 * transaction + 1,
            (2 * transaction).saturating_sub(2),
        ] {
            if ordinal == 2 * transaction {
                continue;
            }
            let foreign = evaluate(
                &read,
                &read_result(&read.input_type, ordinal, 512, false, &[0x5a; 512]),
            );
            let fault = transition(&accept, &advance, &state, &foreign);
            assert_eq!(tag(&evaluate(&route, &fault)), "stopped");
            assert_eq!(tag(&evaluate(&accepted, &fault)), "refused");
        }
        state = transition(&accept, &advance, &state, &data);
        assert_eq!(phase(&state), 4);
        assert_eq!(tag(&evaluate(&accepted, &state)), "refused");
        let duplicate = transition(&accept, &advance, &state, &data);
        assert_eq!(tag(&evaluate(&route, &duplicate)), "stopped");
        let status = evaluate(
            &read,
            &read_result(&read.input_type, 2 * transaction + 1, 13, false, &CSW),
        );
        let stale = evaluate(
            &read,
            &read_result(&read.input_type, 2 * transaction + 3, 13, false, &CSW),
        );
        assert_eq!(
            tag(&evaluate(
                &route,
                &transition(&accept, &advance, &state, &stale)
            )),
            "stopped"
        );
        for terminal_name in ["stalled", "provider-lost", "unsupported", "timeout"] {
            let event = evaluate(&read, &terminal(&read.input_type, terminal_name));
            let fault = transition(&accept, &advance, &state, &event);
            assert_eq!(tag(&evaluate(&route, &fault)), "stopped");
            assert_eq!(tag(&evaluate(&accepted, &fault)), "refused");
        }
        state = transition(&accept, &advance, &state, &status);
        assert_eq!(
            scalar(member(&state, "transaction")),
            (transaction + 1).to_le_bytes()
        );
        assert_eq!(tag(&evaluate(&accepted, &state)), "ready");
        assert_eq!(
            tag(&evaluate(&outcome, &state)),
            if transaction == 15 {
                "accepted"
            } else {
                "refused"
            }
        );
    }
    assert_eq!(phase(&state), 5);
    assert_eq!(tag(&evaluate(&route, &state)), "stopped");
    assert_eq!(scalar(member(&state, "terminal")), [1]);
    let extra = evaluate(&write, &write_result(&write.input_type, 31));
    let refused = transition(&accept, &advance, &state, &extra);
    assert_eq!(phase(&refused), 5);
    assert_eq!(tag(&evaluate(&route, &refused)), "stopped");
    assert_eq!(tag(&evaluate(&accepted, &refused)), "refused");
}
