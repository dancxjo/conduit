//! Production-kernel ordering, output pressure, and explicit terminal outcomes.
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;

fn command(ty: &StructuredInfoType, ordinal: Option<u64>, keyboard: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("command")
    };
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
    let observation_type = cases
        .iter()
        .find(|c| c.tag() == "observation")
        .unwrap()
        .payload_type();
    let StructuredInfoTypeShape::Record { fields, .. } = observation_type.shape() else {
        panic!("observation")
    };
    let field_type = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let observed_type = field_type("observed");
    let observed = if keyboard && ordinal != 1 {
        let StructuredInfoTypeShape::Variant { cases, .. } = observed_type.shape() else {
            panic!("observed")
        };
        let report_type = cases
            .iter()
            .find(|c| c.tag() == "keyboard")
            .unwrap()
            .payload_type();
        let StructuredInfoTypeShape::Record { fields, .. } = report_type.shape() else {
            panic!("report")
        };
        let keys_type = fields
            .iter()
            .find(|f| f.name() == "keys")
            .unwrap()
            .value_type();
        let StructuredInfoTypeShape::Collection { element, .. } = keys_type.shape() else {
            panic!("keys")
        };
        let keys = match ordinal {
            0 => [9, 4, 0, 0, 0, 0],
            2 => [5, 4, 0, 0, 0, 0],
            _ => [0; 6],
        };
        let report = StructuredInfoValue::record(
            report_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "modifiers",
                    StructuredInfoValue::leaf(
                        fields
                            .iter()
                            .find(|f| f.name() == "modifiers")
                            .unwrap()
                            .value_type()
                            .clone(),
                        vec![0],
                    )
                    .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "keys",
                    StructuredInfoValue::collection(
                        keys_type.clone(),
                        keys.into_iter()
                            .map(|key| {
                                StructuredInfoValue::leaf(element.clone(), vec![key]).unwrap()
                            })
                            .collect(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        StructuredInfoValue::variant(observed_type, "keyboard", report).unwrap()
    } else {
        StructuredInfoValue::variant(observed_type, "short", unit()).unwrap()
    };
    let observation = StructuredInfoValue::record(
        observation_type.clone(),
        vec![
            StructuredFieldValue::new(
                "ordinal",
                StructuredInfoValue::leaf(field_type("ordinal"), ordinal.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("observed", observed).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(ty.clone(), "observation", observation)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn execute(
    ordinals: &[u64],
    expected_count: u64,
    terminal: &str,
    pressure: bool,
    keyboard: bool,
    endpoint: bool,
) {
    let entry_name = if endpoint {
        "usb-hid-keyboard-ordered-results"
    } else {
        "usb-hid-keyboard-order-lifecycle"
    };
    let (schemas, output_schemas, mut run) = super::state_kernel::prepared_package_schemas(
        &super::order_lifecycle::package(),
        entry_name,
    );
    let boundary = &run.kernel().definition().boundary;
    let begin = boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "begin")
        .unwrap()
        .external_port
        .clone();
    let input = boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == if endpoint { "result" } else { "command" })
        .unwrap()
        .external_port
        .clone();
    let observation = boundary
        .output_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "observation")
        .unwrap()
        .external_port
        .clone();
    let ended = boundary
        .output_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "ended")
        .unwrap()
        .external_port
        .clone();
    let changes = boundary
        .output_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "changes")
        .unwrap()
        .external_port
        .clone();
    let decoder = conduitos::source_keyboard_batch::SourceKeyboardBatchDecoder::prepare(
        output_schemas.get(&changes.port_id).unwrap(),
    )
    .unwrap();
    let mut batch = ValuePayload {
        value_kind: changes.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let expected_batches: [&[(u8, bool)]; 3] = [
        &[(4, true), (9, true)],
        &[(9, false), (5, true)],
        &[(4, false), (5, false)],
    ];
    let mut batches = 0;
    let mut held_batch_steps = 0;
    let ty = schemas.get(&input.port_id).unwrap();
    let contract =
        conduitos::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap();
    let mut encoder =
        conduitos::usb_base::endpoint_read_result::PreparedEndpointReadResultEncoder::new(
            &contract,
        )
        .unwrap();
    let mut inputs: Vec<_> = ordinals
        .iter()
        .copied()
        .map(Some)
        .chain((!endpoint && matches!(terminal, "closed" | "gap")).then_some(None))
        .map(|ordinal| ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: if endpoint {
                let ordinal = ordinal.unwrap();
                let wire = match ordinal {
                    0 => [0, 0, 9, 4, 0, 0, 0, 0],
                    2 => [0, 0, 5, 4, 0, 0, 0, 0],
                    _ => [0; 8],
                };
                let actual = if !keyboard || ordinal == 1 { 0 } else { 8 };
                encoder
                    .completed(ordinal, 8, actual, &wire[..usize::from(actual)])
                    .unwrap()
                    .to_vec()
            } else {
                command(ty, ordinal, keyboard)
            },
        })
        .collect();
    if endpoint
        && matches!(
            terminal,
            "stalled" | "provider-lost" | "timeout" | "unsupported"
        )
    {
        use conduitos::usb_base::endpoint_read_result::EndpointReadDisposition;
        let disposition = match terminal {
            "stalled" => EndpointReadDisposition::Stalled,
            "provider-lost" => EndpointReadDisposition::ProviderLost,
            "timeout" => EndpointReadDisposition::Timeout,
            _ => EndpointReadDisposition::Unsupported,
        };
        inputs.push(ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: encoder.disposition(disposition).unwrap().to_vec(),
        });
    }
    let seed = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let mut output = ValuePayload {
        value_kind: observation.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut end = ValuePayload {
        value_kind: ended.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    run.start().unwrap();
    assert!(matches!(
        run.admit_input(&begin.port_id, 0, &seed).unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    run.close_input(&begin.port_id).unwrap();
    let mut sent = 0;
    let mut received = 0_u64;
    let mut held_steps = 0;
    let mut saw_end = false;
    let mut finished = false;
    let allocations = crate::allocation::allocations(|| {
        for _ in 0..20_000 {
            if sent < inputs.len()
                && matches!(
                    run.admit_input(&input.port_id, sent as u64, &inputs[sent])
                        .unwrap(),
                    RemoteIngressOutcome::Accepted { .. }
                )
            {
                sent += 1;
                if sent == inputs.len() {
                    run.close_input(&input.port_id).unwrap();
                }
            }
            let status = run.step().unwrap();
            if let Some(sequence) = run.output_into(&observation.port_id, &mut output).unwrap() {
                let value = validate_canonical_structured_value(&output.encoded).unwrap();
                assert_eq!(sequence, received);
                assert_eq!(
                    value
                        .record_field("ordinal")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    received.to_le_bytes()
                );
                assert!(
                    value
                        .record_field("observed")
                        .unwrap()
                        .unwrap()
                        .variant_payload(if keyboard && received != 1 {
                            "keyboard"
                        } else {
                            "short"
                        })
                        .unwrap()
                        .is_some()
                );
                if pressure && received == 0 && held_steps < 128 {
                    held_steps += 1;
                    continue;
                }
                run.complete_output(&observation.port_id, sequence).unwrap();
                received += 1;
            }
            if let Some(sequence) = run.output_into(&changes.port_id, &mut batch).unwrap() {
                assert!(keyboard);
                assert_eq!(sequence, batches as u64);
                if batches == 0 && held_batch_steps < 128 {
                    held_batch_steps += 1;
                    continue;
                }
                let decoded = decoder.decode(&batch.encoded).unwrap();
                assert_eq!(decoded.transitions().len(), expected_batches[batches].len());
                for (actual, &(usage, pressed)) in
                    decoded.transitions().iter().zip(expected_batches[batches])
                {
                    assert_eq!(
                        (actual.usage(), actual.pressed(), actual.modifiers()),
                        (usage, pressed, 0)
                    );
                }
                run.complete_output(&changes.port_id, sequence).unwrap();
                batches += 1;
            }
            if let Some(sequence) = run.output_into(&ended.port_id, &mut end).unwrap() {
                assert!(!saw_end);
                let end_value = validate_canonical_structured_value(&end.encoded).unwrap();
                let actual_end = [
                    "closed",
                    "gap",
                    "stale",
                    "duplicate",
                    "outside-window",
                    "pressure",
                    "stalled",
                    "provider-lost",
                    "timeout",
                    "unsupported",
                ]
                .into_iter()
                .find(|tag| end_value.variant_payload(tag).unwrap().is_some())
                .unwrap();
                assert_eq!(actual_end, terminal);
                assert_eq!(received, expected_count);
                run.complete_output(&ended.port_id, sequence).unwrap();
                saw_end = true;
            }
            if status == KernelCompositeStatus::Complete {
                assert_eq!(
                    run.output_terminal_into(&observation.port_id, &mut output)
                        .unwrap(),
                    Some(KernelCompositeTerminal::Normal)
                );
                assert_eq!(
                    run.output_terminal_into(&ended.port_id, &mut end).unwrap(),
                    Some(KernelCompositeTerminal::Normal)
                );
                assert_eq!(
                    run.output_terminal_into(&changes.port_id, &mut batch)
                        .unwrap(),
                    Some(KernelCompositeTerminal::Normal)
                );
                finished = true;
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert!(finished && saw_end);
    assert_eq!(received, expected_count);
    assert_eq!(batches, if keyboard { 3 } else { 0 });
    assert_eq!(held_batch_steps, if keyboard { 128 } else { 0 });
    assert_eq!(held_steps, if pressure { 128 } else { 0 });
}

#[test]
fn ordered_source_drains_eight_observations_under_pressure_without_allocations() {
    execute(&[7, 3, 0, 6, 1, 5, 2, 4], 8, "closed", true, false, false);
}

#[test]
fn gaps_duplicates_stale_and_distant_ordinals_remain_explicit() {
    execute(&[1], 0, "gap", false, false, false);
    execute(&[1, 1], 0, "duplicate", false, false, false);
    execute(&[0, 0], 1, "stale", false, false, false);
    execute(&[8], 0, "outside-window", false, false, false);
}

#[test]
fn keyboard_history_follows_wire_order_and_skips_invalid_reports_under_batch_pressure() {
    execute(&[2, 1, 3, 0], 4, "closed", true, true, false);
}

#[test]
fn completed_endpoint_octets_reach_ordered_keyboard_batches_in_one_kernel() {
    execute(&[2, 1, 3, 0], 4, "closed", true, true, true);
}

#[test]
fn endpoint_physical_failures_follow_completed_prefix_and_remain_explicit() {
    for terminal in ["stalled", "provider-lost", "timeout", "unsupported"] {
        execute(&[0], 1, terminal, false, false, true);
    }
}
