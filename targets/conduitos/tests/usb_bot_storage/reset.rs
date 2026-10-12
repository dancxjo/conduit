//! Checked recovery policy; execution/quiescence integration is separate.
use super::*;
fn source() -> PreparedProtocolSource {
    PreparedProtocolSource::prepare(
        ProtocolSourcePackage::compile(
            include_str!("../../plots/usb/mass-storage-reset-policy.conduit").into(),
            &serde_json::from_str::<Vec<ProtocolSpecializationRequest>>(include_str!(
                "../../plots/usb/mass-storage-reset-specializations.json"
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn evaluate(p: &PortableExpressionProgram, v: &StructuredInfoValue) -> Vec<u8> {
    let canonical = v.canonical_bytes().unwrap();
    let input = if matches!(p.input_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
        validate_canonical_structured_value(&canonical)
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap()
            .to_vec()
    } else {
        canonical
    };
    let portable = p.evaluate(&input).unwrap();
    let mut native = conduit_plot::PreparedPortableExpressionEvaluator::new(p).unwrap();
    assert_eq!(native.evaluate(&input).unwrap(), portable);
    portable
}
fn acknowledgment(
    ty: &StructuredInfoType,
    transferred: u16,
    short: bool,
    data: &[u8],
) -> StructuredInfoValue {
    variant(ty, "completed", |payload| {
        record(payload, |part| match part.name() {
            "transferred" => StructuredInfoValue::leaf(
                part.value_type().clone(),
                transferred.to_le_bytes().to_vec(),
            )
            .unwrap(),
            "short" => {
                StructuredInfoValue::leaf(part.value_type().clone(), vec![u8::from(short)]).unwrap()
            }
            "input" => {
                let StructuredInfoTypeShape::Sequence { element, .. } = part.value_type().shape()
                else {
                    panic!("sequence")
                };
                StructuredInfoValue::sequence(
                    part.value_type().clone(),
                    data.iter()
                        .map(|b| StructuredInfoValue::leaf(element.clone(), vec![*b]).unwrap())
                        .collect(),
                )
                .unwrap()
            }
            _ => unreachable!(),
        })
    })
}
fn request(bytes: &[u8], expected: [u8; 8]) {
    let value = validate_canonical_structured_value(bytes).unwrap();
    assert_eq!(value.variant_tag().unwrap(), "ready");
    let payload = value.variant_payload("ready").unwrap().unwrap();
    assert_eq!(
        payload
            .record_field("setup")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        expected
    );
    assert_eq!(
        payload
            .record_field("output")
            .unwrap()
            .unwrap()
            .collection_length()
            .unwrap(),
        0
    );
}
#[test]
fn reset_and_clear_halt_requests_preserve_exact_usb_wire_order() {
    let s = source();
    let start = program(&s, "bot-recovery-start");
    request(
        &evaluate(&start, &scalar(&start.input_type, 0)),
        [0x21, 0xff, 0, 0, 0, 0, 0, 0],
    );
    for (name, endpoint) in [
        ("bot-recovery-after-reset", 0x81),
        ("bot-recovery-after-clear-in", 0x02),
    ] {
        let p = program(&s, name);
        request(
            &evaluate(&p, &acknowledgment(&p.input_type, 0, false, &[])),
            [2, 1, 0, 0, endpoint, 0, 0, 0],
        );
    }
    let p = program(&s, "bot-recovery-after-clear-out");
    let bytes = evaluate(&p, &acknowledgment(&p.input_type, 0, false, &[]));
    assert_eq!(
        validate_canonical_structured_value(&bytes)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "recovered"
    );
}
#[test]
fn wrong_interface_and_every_failed_or_malformed_ack_refuse_next_effect() {
    let s = source();
    let start = program(&s, "bot-recovery-start");
    for interface in [1, 255, u64::MAX] {
        let bytes = evaluate(&start, &scalar(&start.input_type, interface));
        assert_eq!(
            validate_canonical_structured_value(&bytes)
                .unwrap()
                .variant_tag()
                .unwrap(),
            "refused"
        );
    }
    for name in [
        "bot-recovery-after-reset",
        "bot-recovery-after-clear-in",
        "bot-recovery-after-clear-out",
    ] {
        let p = program(&s, name);
        let mut cases = vec![
            acknowledgment(&p.input_type, 1, false, &[]),
            acknowledgment(&p.input_type, 0, true, &[]),
            acknowledgment(&p.input_type, 0, false, &[0]),
        ];
        let StructuredInfoTypeShape::Variant { cases: tags, .. } = p.input_type.shape() else {
            panic!("variant")
        };
        for tag in tags.iter().filter(|t| t.tag() != "completed") {
            cases.push(variant(&p.input_type, tag.tag(), |unit| {
                StructuredInfoValue::leaf(unit.clone(), vec![]).unwrap()
            }));
        }
        for case in cases {
            let bytes = evaluate(&p, &case);
            assert_eq!(
                validate_canonical_structured_value(&bytes)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "refused",
                "{name}"
            );
        }
    }
}

#[test]
fn sequenced_graph_uses_one_control_owner_and_stops_after_three_acks_or_first_failure() {
    let s = source();
    let expanded = s.expand("bot-reset-recovery").unwrap();
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "machine/usb/control")
            .count(),
        1
    );
    assert!(!expanded
        .expanded
        .gears
        .iter()
        .any(|g| g.kind_id.as_str().starts_with("storage/")
            || g.kind_id.as_str().contains("endpoint-write")));
    let advance = program(&s, "bot-recovery-advance");
    let state_type = fields(&advance.input_type)[0].value_type();
    let result_type = fields(&advance.input_type)[1].value_type();
    for phase in 0..3 {
        for failed in [false, true] {
            let state = record(state_type, |f| match f.name() {
                "phase" => scalar(f.value_type(), phase),
                _ => StructuredInfoValue::leaf(f.value_type().clone(), vec![0]).unwrap(),
            });
            let ack = acknowledgment(result_type, 0, failed, &[]);
            let pair = record(&advance.input_type, |f| {
                if f.name() == fields(&advance.input_type)[0].name() {
                    state.clone()
                } else {
                    ack.clone()
                }
            });
            let bytes = evaluate(&advance, &pair);
            let value = validate_canonical_structured_value(&bytes).unwrap();
            assert_eq!(
                value
                    .record_field("terminal")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [u8::from(failed || phase == 2)]
            );
            assert_eq!(
                value
                    .record_field("failed")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [u8::from(failed)]
            );
        }
    }
}
