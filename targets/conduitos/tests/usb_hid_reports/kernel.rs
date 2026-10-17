use super::kernel_fixture::Execution;
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::{ValuePayload, validate_canonical_structured_value};
use conduit_kernel::scheduler::RemoteIngressOutcome;

#[test]
fn checked_keyboard_class_sorts_64_frames_and_closes_without_allocations() {
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
    assert_eq!(outputs.len(), 2);
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: crate::usb_hid_reports::common::frame(
            &run.input_schema,
            &[0x5a, 0, 255, 4, 9, 8, 7, 6],
            8,
        ),
    };
    let mut slots: Vec<_> = outputs
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let mut complete = false;
    let allocations = crate::allocation::allocations(|| {
        run.kernel.start().unwrap();
        for sequence in 0..64 {
            assert_eq!(
                run.kernel
                    .admit_input(&input_port.port_id, sequence, &input)
                    .unwrap(),
                RemoteIngressOutcome::Accepted { sequence }
            );
            if sequence == 63 {
                run.kernel.close_input(&input_port.port_id).unwrap();
            }
            let mut seen = [false; 2];
            for _ in 0..256 {
                let status = run.step();
                for (index, port) in outputs.iter().enumerate() {
                    if let Some(actual) = run
                        .kernel
                        .output_into(&port.port_id, &mut slots[index])
                        .unwrap()
                    {
                        assert_eq!(actual, sequence);
                        assert!(!seen[index]);
                        seen[index] = true;
                        let value =
                            validate_canonical_structured_value(&slots[index].encoded).unwrap();
                        let report = if port.port_id.as_str() == "observed" {
                            value.variant_payload("keyboard").unwrap().unwrap()
                        } else {
                            value
                        };
                        assert_eq!(
                            report
                                .record_field("modifiers")
                                .unwrap()
                                .unwrap()
                                .primitive_bytes("value/u8")
                                .unwrap(),
                            [0x5a]
                        );
                        let keys = report.record_field("keys").unwrap().unwrap();
                        let expected = if port.port_id.as_str() == "decoded" {
                            [4, 6, 7, 8, 9, 255]
                        } else {
                            [255, 4, 9, 8, 7, 6]
                        };
                        for (at, key) in expected.iter().enumerate() {
                            assert_eq!(
                                keys.collection_index(u16::try_from(at).unwrap())
                                    .unwrap()
                                    .unwrap()
                                    .primitive_bytes("value/u8")
                                    .unwrap(),
                                [*key]
                            );
                        }
                        run.kernel.complete_output(&port.port_id, actual).unwrap();
                    }
                }
                if status == KernelCompositeStatus::Complete {
                    complete = true;
                    break;
                }
                if sequence < 63 && seen.iter().all(|seen| *seen) {
                    break;
                }
            }
            assert!(seen.iter().all(|seen| *seen));
        }
        assert!(complete);
        for (index, port) in outputs.iter().enumerate() {
            assert_eq!(
                run.kernel
                    .output_terminal_into(&port.port_id, &mut slots[index])
                    .unwrap(),
                Some(KernelCompositeTerminal::Normal)
            );
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn checked_mouse_class_decodes_64_frames_and_closes_without_allocations() {
    let mut run = Execution::prepare("usb-hid-boot-mouse");
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let outputs = &run.kernel.definition().boundary.output_fronts;
    assert_eq!(outputs.len(), 1);
    let output_port = outputs[0].external_port.clone();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: crate::usb_hid_reports::common::frame(
            &run.input_schema,
            &[0xf9, 127, 255, 0xab],
            4,
        ),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut complete = false;
    let allocations = crate::allocation::allocations(|| {
        run.kernel.start().unwrap();
        for sequence in 0..64 {
            assert_eq!(
                run.kernel
                    .admit_input(&input_port.port_id, sequence, &input)
                    .unwrap(),
                RemoteIngressOutcome::Accepted { sequence }
            );
            if sequence == 63 {
                run.kernel.close_input(&input_port.port_id).unwrap();
            }
            let mut seen = false;
            for _ in 0..128 {
                let status = run.step();
                if let Some(actual) = run
                    .kernel
                    .output_into(&output_port.port_id, &mut output)
                    .unwrap()
                {
                    assert!(!seen);
                    seen = true;
                    assert_eq!(actual, sequence);
                    let report = validate_canonical_structured_value(&output.encoded)
                        .unwrap()
                        .variant_payload("mouse")
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        report
                            .record_field("buttons")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u8")
                            .unwrap(),
                        [1]
                    );
                    assert_eq!(
                        report
                            .record_field("x")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/i16")
                            .unwrap(),
                        127_i16.to_le_bytes()
                    );
                    assert_eq!(
                        report
                            .record_field("y")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/i16")
                            .unwrap(),
                        (-1_i16).to_le_bytes()
                    );
                    assert_eq!(
                        report
                            .record_field("wire")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/bytes")
                            .unwrap()[3],
                        0xab
                    );
                    run.kernel
                        .complete_output(&output_port.port_id, actual)
                        .unwrap();
                }
                if status == KernelCompositeStatus::Complete {
                    complete = true;
                    break;
                }
                if sequence < 63 && seen {
                    break;
                }
            }
            assert!(seen);
        }
        assert!(complete);
        assert_eq!(
            run.kernel
                .output_terminal_into(&output_port.port_id, &mut output)
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}
