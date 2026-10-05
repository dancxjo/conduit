//! A held class output cannot be silently replaced by a later report.
use super::kernel_fixture::Execution;
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::{ValuePayload, port_id, validate_canonical_structured_value};
use conduit_kernel::scheduler::RemoteIngressOutcome;

#[test]
fn held_outputs_preserve_order_and_a_bad_report_emits_only_its_typed_observation() {
    let mut run = Execution::prepare("usb-hid-boot-keyboard-zero-reserved");
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let outputs: Vec<_> = run
        .kernel
        .definition()
        .boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    let observed = outputs
        .iter()
        .position(|port| port.port_id == port_id("observed"))
        .unwrap();
    let decoded = outputs
        .iter()
        .position(|port| port.port_id == port_id("decoded"))
        .unwrap();
    let input = |wire: &[u8], actual| ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: crate::descriptor_frame::frame(&run.input_schema, wire, actual),
    };
    let first = input(&[0, 0, 9, 0, 0, 0, 0, 0], 8);
    let bad = input(&[0; 7], 8);
    let last = input(&[0, 0, 4, 0, 0, 0, 0, 0], 8);
    let mut slots: Vec<_> = outputs
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    run.kernel.start().unwrap();
    assert_eq!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &first)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    );
    for _ in 0..256 {
        run.step();
        if outputs.iter().enumerate().all(|(at, port)| {
            run.kernel
                .output_into(&port.port_id, &mut slots[at])
                .unwrap()
                == Some(0)
        }) {
            break;
        }
    }
    for (at, port) in outputs.iter().enumerate() {
        assert_eq!(
            run.kernel
                .output_into(&port.port_id, &mut slots[at])
                .unwrap(),
            Some(0)
        );
    }
    let retained: Vec<_> = slots.iter().map(|slot| slot.encoded.clone()).collect();
    assert_eq!(
        run.kernel
            .admit_input(&input_port.port_id, 1, &bad)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 1 }
    );
    for _ in 0..128 {
        run.step();
    }
    for (at, port) in outputs.iter().enumerate() {
        assert_eq!(
            run.kernel
                .output_into(&port.port_id, &mut slots[at])
                .unwrap(),
            Some(0)
        );
        assert_eq!(slots[at].encoded, retained[at]);
        run.kernel.complete_output(&port.port_id, 0).unwrap();
    }
    for _ in 0..256 {
        run.step();
        if run
            .kernel
            .output_into(&outputs[observed].port_id, &mut slots[observed])
            .unwrap()
            == Some(1)
        {
            break;
        }
    }
    assert_eq!(
        run.kernel
            .output_into(&outputs[observed].port_id, &mut slots[observed])
            .unwrap(),
        Some(1)
    );
    assert!(
        validate_canonical_structured_value(&slots[observed].encoded)
            .unwrap()
            .variant_payload("malformed")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        run.kernel
            .output_into(&outputs[decoded].port_id, &mut slots[decoded])
            .unwrap(),
        None
    );
    run.kernel
        .complete_output(&outputs[observed].port_id, 1)
        .unwrap();
    assert_eq!(
        run.kernel
            .admit_input(&input_port.port_id, 2, &last)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 2 }
    );
    run.kernel.close_input(&input_port.port_id).unwrap();
    let mut seen = [false; 2];
    let mut complete = false;
    for _ in 0..256 {
        let status = run.step();
        for (at, port) in outputs.iter().enumerate() {
            if let Some(sequence) = run
                .kernel
                .output_into(&port.port_id, &mut slots[at])
                .unwrap()
            {
                assert!(!seen[at]);
                seen[at] = true;
                assert_eq!(sequence, if at == observed { 2 } else { 1 });
                if at == decoded {
                    let value = validate_canonical_structured_value(&slots[at].encoded).unwrap();
                    assert_eq!(
                        value
                            .record_field("keys")
                            .unwrap()
                            .unwrap()
                            .collection_index(5)
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u8")
                            .unwrap(),
                        [4]
                    );
                }
                run.kernel.complete_output(&port.port_id, sequence).unwrap();
            }
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(complete && seen.iter().all(|seen| *seen));
    for (at, port) in outputs.iter().enumerate() {
        assert_eq!(
            run.kernel
                .output_terminal_into(&port.port_id, &mut slots[at])
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    }
}
