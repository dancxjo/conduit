//! Production-kernel ordering, output pressure, and explicit terminal outcomes.
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;

fn command(ty: &StructuredInfoType, ordinal: Option<u64>) -> Vec<u8> {
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
    let observation = StructuredInfoValue::record(
        observation_type.clone(),
        vec![
            StructuredFieldValue::new(
                "ordinal",
                StructuredInfoValue::leaf(field_type("ordinal"), ordinal.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "observed",
                StructuredInfoValue::variant(field_type("observed"), "short", unit()).unwrap(),
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

fn execute(ordinals: &[u64], expected_count: u64, terminal: &str, pressure: bool) {
    let (schemas, mut run) = super::state_kernel::prepared_package(
        &super::order_lifecycle::package(),
        "usb-hid-keyboard-order-lifecycle",
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
        .find(|p| p.external_port.port_id.as_str() == "command")
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
    let ty = schemas.get(&input.port_id).unwrap();
    let inputs: Vec<_> = ordinals
        .iter()
        .copied()
        .map(Some)
        .chain(matches!(terminal, "closed" | "gap").then_some(None))
        .map(|ordinal| ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: command(ty, ordinal),
        })
        .collect();
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
                        .variant_payload("short")
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
            if let Some(sequence) = run.output_into(&ended.port_id, &mut end).unwrap() {
                assert!(!saw_end);
                assert_eq!(received, expected_count);
                super::common::tag(&end.encoded, terminal);
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
                finished = true;
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert!(finished && saw_end);
    assert_eq!(received, expected_count);
    assert_eq!(held_steps, if pressure { 128 } else { 0 });
}

#[test]
fn ordered_source_drains_eight_observations_under_pressure_without_allocations() {
    execute(&[7, 3, 0, 6, 1, 5, 2, 4], 8, "closed", true);
}

#[test]
fn gaps_duplicates_stale_and_distant_ordinals_remain_explicit() {
    execute(&[1], 0, "gap", false);
    execute(&[1, 1], 0, "duplicate", false);
    execute(&[0, 0], 1, "stale", false);
    execute(&[8], 0, "outside-window", false);
}
