//! Ordered mouse lifecycle through the sole kernel, without device effects.
#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;
#[path = "usb_mouse_order/common.rs"]
mod common;
use common::*;
use conduit_composite::{KernelCompositeError, KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::protocol_host_calls::ProtocolCallRefusal;

fn command(ty: &StructuredInfoType, ordinal: Option<u64>, terminal: &str) -> Vec<u8> {
    let unit = || {
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
            vec![],
        )
        .unwrap()
    };
    let Some(ordinal) = ordinal else {
        return StructuredInfoValue::variant(ty.clone(), terminal, unit())
            .unwrap()
            .canonical_bytes()
            .unwrap();
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("command")
    };
    let observation_type = cases
        .iter()
        .find(|case| case.tag() == "observation")
        .unwrap()
        .payload_type();
    let observation = StructuredInfoValue::record(
        observation_type.clone(),
        vec![
            leaf_field(observation_type, "ordinal", &ordinal.to_le_bytes()),
            StructuredFieldValue::new(
                "observed",
                StructuredInfoValue::variant(
                    field_type(observation_type, "observed"),
                    "short",
                    unit(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(ty.clone(), "observation", observation)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn execute(ordinals: &[u64], expected: u64, terminal: &str, pressure: bool) {
    let (schema, mut run) = prepared();
    let boundary = &run.kernel().definition().boundary;
    let input = |name: &str| {
        boundary
            .input_fronts
            .iter()
            .find(|port| port.external_port.port_id.as_str() == name)
            .unwrap()
            .external_port
            .clone()
    };
    let output = |name: &str| {
        boundary
            .output_fronts
            .iter()
            .find(|port| port.external_port.port_id.as_str() == name)
            .unwrap()
            .external_port
            .clone()
    };
    let begin = input("begin");
    let input = input("command");
    let observation = output("event");
    let final_command = match terminal {
        "closed" | "gap" => Some("finish"),
        "stalled" | "provider-lost" | "timeout" | "unsupported" => Some(terminal),
        _ => None,
    };
    let inputs: Vec<_> = ordinals
        .iter()
        .copied()
        .map(Some)
        .chain(final_command.map(|_| None))
        .map(|ordinal| ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: command(&schema, ordinal, final_command.unwrap_or("finish")),
        })
        .collect();
    let seed = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let mut observed = ValuePayload {
        value_kind: observation.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut next = 0;
    let mut received = 0_u64;
    let mut held = 0;
    let mut saw_end = false;
    let mut complete = false;
    let allocations = allocation::allocations(|| {
        run.start().unwrap();
        assert!(matches!(
            run.admit_input(&begin.port_id, 0, &seed).unwrap(),
            RemoteIngressOutcome::Accepted { .. }
        ));
        run.close_input(&begin.port_id).unwrap();
        for _ in 0..20000 {
            if next < inputs.len() {
                match run
                    .admit_input(&input.port_id, next as u64, &inputs[next])
                    .unwrap()
                {
                    RemoteIngressOutcome::Accepted { .. } => {
                        next += 1;
                        if next == inputs.len() {
                            run.close_input(&input.port_id).unwrap();
                        }
                    }
                    RemoteIngressOutcome::Full { .. } => {}
                }
            }
            let status = run.step().unwrap();
            if let Some(sequence) = run
                .output_into(&observation.port_id, &mut observed)
                .unwrap()
            {
                let event = validate_canonical_structured_value(&observed.encoded).unwrap();
                if let Some(value) = event.variant_payload("observation").unwrap() {
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
                            .variant_payload("short")
                            .unwrap()
                            .is_some()
                    );
                    if pressure && received == 0 && held < 128 {
                        held += 1;
                    } else {
                        received += 1;
                        run.complete_output(&observation.port_id, sequence).unwrap();
                    }
                } else {
                    assert!(!saw_end);
                    assert_eq!(
                        received, expected,
                        "terminal cannot overtake held observations"
                    );
                    assert_eq!(sequence, expected);
                    assert!(
                        event
                            .variant_payload("ended")
                            .unwrap()
                            .unwrap()
                            .variant_payload(terminal)
                            .unwrap()
                            .is_some()
                    );
                    saw_end = true;
                    run.complete_output(&observation.port_id, sequence).unwrap();
                }
            }
            if status == KernelCompositeStatus::Complete {
                assert_eq!(
                    run.output_terminal_into(&observation.port_id, &mut observed)
                        .unwrap(),
                    Some(KernelCompositeTerminal::Normal)
                );
                complete = true;
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert!(complete && saw_end);
    assert_eq!(received, expected);
    assert_eq!(held, if pressure { 128 } else { 0 });
}

#[test]
fn ordered_observations_survive_output_pressure_and_close_normally() {
    execute(&[1, 0, 3, 2], 4, "closed", true);
}

#[test]
fn missing_duplicate_stale_and_distant_ordinals_have_distinct_endings() {
    execute(&[1], 0, "gap", false);
    execute(&[1, 1], 0, "duplicate", false);
    execute(&[0, 0], 1, "stale", false);
    execute(&[2], 0, "outside-window", false);
    execute(&[u64::MAX], 0, "outside-window", false);
}

#[test]
fn ordering_reuses_the_same_two_slots_across_sixty_four_cycles() {
    let ordinals: Vec<_> = (0..64_u64)
        .flat_map(|cycle| [cycle * 2 + 1, cycle * 2])
        .collect();
    execute(&ordinals, 128, "closed", false);
}

#[test]
fn physical_failures_preserve_the_completed_prefix_and_remain_distinct() {
    for terminal in ["stalled", "provider-lost", "timeout", "unsupported"] {
        execute(&[0], 1, terminal, false);
    }
}

#[test]
fn cancellation_revokes_a_held_observation_without_a_normal_finish() {
    let (schema, mut run) = prepared();
    let boundary = &run.kernel().definition().boundary;
    let begin = boundary
        .input_fronts
        .iter()
        .find(|port| port.external_port.port_id.as_str() == "begin")
        .unwrap()
        .external_port
        .clone();
    let input = boundary
        .input_fronts
        .iter()
        .find(|port| port.external_port.port_id.as_str() == "command")
        .unwrap()
        .external_port
        .clone();
    let output = boundary
        .output_fronts
        .iter()
        .find(|port| port.external_port.port_id.as_str() == "event")
        .unwrap()
        .external_port
        .clone();
    let seed = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let report = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: command(&schema, Some(0), "finish"),
    };
    let mut value = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    run.start().unwrap();
    assert!(matches!(
        run.admit_input(&begin.port_id, 0, &seed).unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    run.close_input(&begin.port_id).unwrap();
    assert!(matches!(
        run.admit_input(&input.port_id, 0, &report).unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    let mut held = false;
    for _ in 0..4096 {
        run.step().unwrap();
        if run.output_into(&output.port_id, &mut value).unwrap() == Some(0) {
            held = true;
            break;
        }
    }
    assert!(held);
    let allocations = allocation::allocations(|| {
        run.cancel().unwrap();
        assert_eq!(run.step().unwrap(), KernelCompositeStatus::Cancelled);
        assert!(matches!(
            run.output_into(&output.port_id, &mut value),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
        assert!(matches!(
            run.complete_output(&output.port_id, 0),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
        assert!(matches!(
            run.admit_input(&input.port_id, 1, &report),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
        assert!(matches!(
            run.output_terminal_into(&output.port_id, &mut value),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
    });
    assert_eq!(allocations, 0);
    assert!(
        run.kernel()
            .signs()
            .values()
            .flatten()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::RunCancelled)
    );
}
