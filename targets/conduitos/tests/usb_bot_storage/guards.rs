//! Eager Boolean evaluation must never select an inactive variant payload.
use super::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

fn unit(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    variant(ty, tag, |empty| {
        StructuredInfoValue::leaf(empty.clone(), vec![]).unwrap()
    })
}
fn request(ty: &StructuredInfoType) -> StructuredInfoValue {
    record(ty, |field| {
        scalar(
            field.value_type(),
            match field.name() {
                "block_size" => 512,
                "count" => 1,
                "lba" => 0,
                _ => unreachable!(),
            },
        )
    })
}
fn state(ty: &StructuredInfoType, bank: &[u8], phase: u64) -> StructuredInfoValue {
    record(ty, |field| match field.name() {
        "bank" => StructuredInfoValue::leaf(field.value_type().clone(), bank.to_vec()).unwrap(),
        "command" => record(field.value_type(), |wire| {
            StructuredInfoValue::leaf(wire.value_type().clone(), bank[..31].to_vec()).unwrap()
        }),
        "request" => request(field.value_type()),
        "data" => unit(field.value_type(), "unsupported"),
        "phase" => scalar(field.value_type(), phase),
        "operations" => scalar(field.value_type(), 0),
        "terminal" => StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap(),
        _ => unreachable!(),
    })
}
fn control(ty: &StructuredInfoType, phase: u64, fault: Option<&str>) -> StructuredInfoValue {
    variant(ty, "control", |payload| {
        record(payload, |field| match field.name() {
            "result" => {
                if let Some(tag) = fault {
                    unit(field.value_type(), tag)
                } else {
                    variant(field.value_type(), "completed", |completion| {
                        record(completion, |part| match part.name() {
                            "transferred" => StructuredInfoValue::leaf(
                                part.value_type().clone(),
                                (if phase == 1 { 32u16 } else { 0 }).to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                            "short" => {
                                StructuredInfoValue::leaf(part.value_type().clone(), vec![0])
                                    .unwrap()
                            }
                            "input" => {
                                let StructuredInfoTypeShape::Sequence { element, .. } =
                                    part.value_type().shape()
                                else {
                                    panic!("sequence")
                                };
                                let bytes: &[u8] = if phase == 1 {
                                    &[
                                        9, 2, 32, 0, 1, 1, 5, 192, 0, 9, 4, 0, 0, 2, 8, 6, 80, 0,
                                        7, 5, 129, 2, 0, 2, 0, 7, 5, 2, 2, 0, 2, 0,
                                    ]
                                } else {
                                    &[]
                                };
                                StructuredInfoValue::sequence(
                                    part.value_type().clone(),
                                    bytes
                                        .iter()
                                        .map(|byte| {
                                            StructuredInfoValue::leaf(element.clone(), vec![*byte])
                                                .unwrap()
                                        })
                                        .collect(),
                                )
                                .unwrap()
                            }
                            _ => unreachable!(),
                        })
                    })
                }
            }
            "facts" => record(field.value_type(), |fact| {
                StructuredInfoValue::leaf(
                    fact.value_type().clone(),
                    vec![u8::from(
                        fault.is_none()
                            && ((phase == 1 && fact.name() == "descriptor")
                                || (phase == 2 && fact.name() == "ack")),
                    )],
                )
                .unwrap()
            }),
            _ => unreachable!(),
        })
    })
}

#[test]
fn early_success_and_terminal_events_have_guarded_payload_access_in_both_evaluators() {
    let package = package();
    let bank = conduit_data::bytes_literal::decode_hex(
        package
            .source
            .split("bytes/literal(\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap(),
    )
    .unwrap();
    let source = PreparedProtocolSource::prepare(package).unwrap();
    let accept = program(&source, "bot-storage-accept");
    let emit = program(&source, "bot-storage-emission");
    let advance = program(&source, "bot-storage-advance");
    let observe = program(&source, "bot-storage-read-event");
    let mut prepared_accept = PreparedPortableExpressionEvaluator::new(&accept).unwrap();
    let mut prepared_emit = PreparedPortableExpressionEvaluator::new(&emit).unwrap();
    let mut prepared_advance = PreparedPortableExpressionEvaluator::new(&advance).unwrap();
    let state_type = fields(&accept.input_type)[0].value_type();
    let event_type = fields(&accept.input_type)[1].value_type();
    let read_event = |fault: Option<&str>| {
        let result = if let Some(tag) = fault {
            unit(&observe.input_type, tag)
        } else {
            csw::frame(&observe.input_type, &[0x80; 512], 0, false)
        };
        let facts = StructuredInfoValue::from_canonical_bytes(
            &observe
                .evaluate(&result.canonical_bytes().unwrap())
                .unwrap(),
        )
        .unwrap();
        variant(event_type, "observation", |_| facts.clone())
    };
    let write_event = |fault: Option<&str>| {
        variant(event_type, "observation", |observation| {
            variant(observation, "write", |result| {
                if let Some(tag) = fault {
                    unit(result, tag)
                } else {
                    variant(result, "completed", |completion| {
                        record(completion, |field| scalar(field.value_type(), 31))
                    })
                }
            })
        })
    };
    let mut cases = vec![
        (0, variant(event_type, "command", request), true, "absent"),
        (1, control(event_type, 1, None), true, "absent"),
        (2, control(event_type, 2, None), true, "absent"),
        (3, write_event(None), true, "absent"),
        (4, read_event(None), true, "absent"),
    ];
    for (fault, expected) in [
        ("stalled", "stalled"),
        ("provider-lost", "provider-lost"),
        ("unsupported", "command-failed"),
    ] {
        cases.push((1, control(event_type, 1, Some(fault)), false, expected));
        cases.push((3, write_event(Some(fault)), false, expected));
        cases.push((4, read_event(Some(fault)), false, expected));
        cases.push((5, read_event(Some(fault)), false, expected));
    }
    cases.push((4, read_event(Some("timeout")), false, "timeout"));
    cases.push((5, read_event(Some("timeout")), false, "timeout"));
    cases.push((
        5,
        variant(event_type, "command", request),
        false,
        "unsupported",
    ));
    for (phase, event, accepted, expected) in cases {
        let previous = state(state_type, &bank, phase);
        let pair = record(&accept.input_type, |field| {
            if field.value_type() == state_type {
                previous.clone()
            } else {
                event.clone()
            }
        })
        .canonical_bytes()
        .unwrap();
        let transition = accept.evaluate(&pair).unwrap();
        assert_eq!(prepared_accept.evaluate(&pair).unwrap(), transition);
        assert_eq!(
            validate_canonical_structured_value(&transition)
                .unwrap()
                .record_field("accepted")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[u8::from(accepted)]
        );
        let output = emit.evaluate(&transition).unwrap();
        assert_eq!(prepared_emit.evaluate(&transition).unwrap(), output);
        let value = validate_canonical_structured_value(&output).unwrap();
        if expected == "absent" {
            assert_eq!(value.variant_tag().unwrap(), "absent");
        } else {
            assert_eq!(
                value
                    .variant_payload("emit")
                    .unwrap()
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                expected
            );
        }
        let next = advance.evaluate(&transition).unwrap();
        assert_eq!(prepared_advance.evaluate(&transition).unwrap(), next);
    }
}
