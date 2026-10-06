//! Source pointer history across ordered class events, through the sole kernel.
use super::{allocation, common::*};
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::source_pointer_sample::SourcePointerSampleDecoder;

fn case_type(ty: &StructuredInfoType, tag: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
        .clone()
}

fn input(ty: &StructuredInfoType, ordinal: Option<u64>, motion: Option<(u8, i16, i16)>) -> Vec<u8> {
    let unit = || {
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
            vec![],
        )
        .unwrap()
    };
    let Some(ordinal) = ordinal else {
        return StructuredInfoValue::variant(ty.clone(), "finish", unit())
            .unwrap()
            .canonical_bytes()
            .unwrap();
    };
    let observation = case_type(ty, "observation");
    let observed = field_type(&observation, "observed");
    let result = match motion {
        Some((buttons, x, y)) => {
            let report = case_type(&observed, "mouse");
            StructuredInfoValue::variant(
                observed,
                "mouse",
                StructuredInfoValue::record(
                    report.clone(),
                    vec![
                        leaf_field(&report, "buttons", &[buttons]),
                        leaf_field(&report, "x", &x.to_le_bytes()),
                        leaf_field(&report, "y", &y.to_le_bytes()),
                    ],
                )
                .unwrap(),
            )
            .unwrap()
        }
        None => StructuredInfoValue::variant(observed, "short", unit()).unwrap(),
    };
    StructuredInfoValue::variant(
        ty.clone(),
        "observation",
        StructuredInfoValue::record(
            observation.clone(),
            vec![
                leaf_field(&observation, "ordinal", &ordinal.to_le_bytes()),
                StructuredFieldValue::new("observed", result).unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn normalized_history_skips_invalid_reports_and_retains_its_final_event_under_pressure() {
    let (schema, output_schema, mut run) =
        prepared_entry("usb-hid-mouse-order-lifecycle", "command", "event");
    let sample_schema = field_type(&case_type(&output_schema, "sample"), "sample");
    let decoder = SourcePointerSampleDecoder::prepare(&sample_schema).unwrap();
    let boundary = &run.kernel().definition().boundary;
    let begin = boundary
        .input_fronts
        .iter()
        .find(|port| port.external_port.port_id.as_str() == "begin")
        .unwrap()
        .external_port
        .clone();
    let event = boundary
        .input_fronts
        .iter()
        .find(|port| port.external_port.port_id.as_str() == "command")
        .unwrap()
        .external_port
        .clone();
    let output = boundary.output_fronts[0].external_port.clone();
    let seed = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let inputs: Vec<_> = [
        input(&schema, Some(0), Some((7, 127, -127))),
        input(&schema, Some(1), None),
        input(&schema, Some(2), Some((0, -127, 127))),
        input(&schema, None, None),
    ]
    .into_iter()
    .map(|encoded| ValuePayload {
        value_kind: event.value_kind.clone(),
        encoded,
    })
    .collect();
    let mut received = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut sample_bytes = Vec::with_capacity(4096);
    let mut next = 0;
    let mut emitted = 0_u64;
    let mut held = 0;
    let mut complete = false;
    let allocations = allocation::allocations(|| {
        run.start().unwrap();
        assert!(matches!(
            run.admit_input(&begin.port_id, 0, &seed).unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        run.close_input(&begin.port_id).unwrap();
        for _ in 0..20000 {
            if next < inputs.len()
                && matches!(
                    run.admit_input(&event.port_id, next as u64, &inputs[next])
                        .unwrap(),
                    RemoteIngressOutcome::Accepted { .. }
                )
            {
                next += 1;
                if next == inputs.len() {
                    run.close_input(&event.port_id).unwrap();
                }
            }
            let status = run.step().unwrap();
            if let Some(sequence) = run.output_into(&output.port_id, &mut received).unwrap() {
                assert_eq!(sequence, emitted);
                let value = validate_canonical_structured_value(&received.encoded).unwrap();
                match emitted {
                    0 | 2 => {
                        let sample = value.variant_payload("sample").unwrap().unwrap();
                        assert_eq!(
                            sample
                                .record_field("ordinal")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/u64")
                                .unwrap(),
                            emitted.to_le_bytes()
                        );
                        let sample = sample.record_field("sample").unwrap().unwrap();
                        sample_bytes.clear();
                        sample_bytes.extend_from_slice(sample.type_bytes());
                        sample_bytes.extend_from_slice(sample.value_node());
                        let sample = decoder.decode(&sample_bytes).unwrap();
                        let (x, y, dx, dy, pressed, sequence) = if emitted == 0 {
                            (1000000, 0, 508000, -508000, true, 1)
                        } else {
                            (492000, 508000, -508000, 508000, false, 2)
                        };
                        assert_eq!(
                            (
                                sample.position_x,
                                sample.position_y,
                                sample.delta_x,
                                sample.delta_y
                            ),
                            (x, y, dx, dy)
                        );
                        assert_eq!(sample.primary_pressed, pressed);
                        assert_eq!(
                            (
                                sample.sequence,
                                sample.queue_capacity,
                                sample.coalesced,
                                sample.dropped
                            ),
                            (sequence, 8, 0, 0)
                        );
                    }
                    1 => {
                        let observation = value.variant_payload("observation").unwrap().unwrap();
                        assert_eq!(
                            observation
                                .record_field("ordinal")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/u64")
                                .unwrap(),
                            1_u64.to_le_bytes()
                        );
                        assert!(
                            observation
                                .record_field("observed")
                                .unwrap()
                                .unwrap()
                                .variant_payload("short")
                                .unwrap()
                                .is_some()
                        );
                    }
                    3 => {
                        assert!(
                            value
                                .variant_payload("ended")
                                .unwrap()
                                .unwrap()
                                .variant_payload("closed")
                                .unwrap()
                                .is_some()
                        );
                    }
                    _ => panic!("unexpected event"),
                }
                if emitted == 0 && held < 512 {
                    held += 1;
                } else {
                    emitted += 1;
                    run.complete_output(&output.port_id, sequence).unwrap();
                }
            }
            if status == KernelCompositeStatus::Complete {
                assert_eq!(
                    run.output_terminal_into(&output.port_id, &mut received)
                        .unwrap(),
                    Some(KernelCompositeTerminal::Normal)
                );
                complete = true;
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!((emitted, held), (4, 512));
    assert!(complete);
}

#[test]
fn internal_outcome_codes_preserve_typed_endings_and_refuse_unknown_codes() {
    let decode = program("usb-hid-mouse-order-end-code");
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&decode).unwrap();
    for (code, expected) in [
        (0_u8, "closed"),
        (1, "gap"),
        (2, "stale"),
        (3, "duplicate"),
        (4, "outside-window"),
        (5, "pressure"),
        (6, "stalled"),
        (7, "provider-lost"),
        (8, "timeout"),
        (9, "unsupported"),
        (10, "invalid-state"),
        (11, "invalid-motion"),
        (12, "sequence-exhausted"),
        (255, "invalid-state"),
    ] {
        let input = StructuredInfoValue::record(
            decode.input_type.clone(),
            vec![leaf_field(&decode.input_type, "code", &[code])],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let allocations = allocation::allocations(|| {
            assert_tag(evaluator.evaluate(&input).unwrap(), expected);
        });
        assert_eq!(allocations, 0);
    }
}
