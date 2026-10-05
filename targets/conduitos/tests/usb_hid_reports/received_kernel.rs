use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;

#[test]
fn malformed_wire_report_is_observed_without_replacing_previous_keyboard_state() {
    let (schemas, mut run) = super::state_kernel::prepared("usb-hid-keyboard-received");
    let frame_type = schemas.get(&PortId::from("frame")).unwrap().clone();
    let boundary = &run.kernel().definition().boundary;
    let begin = boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "begin")
        .unwrap()
        .external_port
        .clone();
    let frame = boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "frame")
        .unwrap()
        .external_port
        .clone();
    let observed = boundary
        .output_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "observed")
        .unwrap()
        .external_port
        .clone();
    let transition = boundary
        .output_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "transition")
        .unwrap()
        .external_port
        .clone();
    let inputs: Vec<_> = [
        [1, 0, 9, 4, 0, 0, 0, 0],
        [0, 0, 4, 4, 0, 0, 0, 0],
        [0, 0, 5, 4, 0, 0, 0, 0],
    ]
    .iter()
    .map(|wire| ValuePayload {
        value_kind: frame.value_kind.clone(),
        encoded: crate::descriptor_frame::frame(&frame_type, wire, 8),
    })
    .collect();
    let begin_value = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let mut observation = ValuePayload {
        value_kind: observed.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut event = ValuePayload {
        value_kind: transition.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let expected = [
        (224, true, 1),
        (4, true, 1),
        (9, true, 1),
        (224, false, 0),
        (9, false, 0),
        (5, true, 0),
    ];
    let tags = ["keyboard", "duplicate", "keyboard"];
    let mut next_input = 0;
    let mut next_observation = 0;
    let mut next_event = 0;
    let mut held_output_steps = 0;
    let allocations = crate::allocation::allocations(|| {
        run.start().unwrap();
        assert_eq!(
            run.admit_input(&begin.port_id, 0, &begin_value).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        run.close_input(&begin.port_id).unwrap();
        let mut complete = false;
        for _ in 0..8192 {
            if next_input < inputs.len()
                && matches!(
                    run.admit_input(&frame.port_id, next_input as u64, &inputs[next_input])
                        .unwrap(),
                    RemoteIngressOutcome::Accepted { .. }
                )
            {
                next_input += 1;
                if next_input == inputs.len() {
                    run.close_input(&frame.port_id).unwrap();
                }
            }
            run.step().unwrap();
            if let Some(sequence) = run
                .output_into(&observed.port_id, &mut observation)
                .unwrap()
            {
                assert_eq!(sequence, next_observation as u64);
                super::common::tag(&observation.encoded, tags[next_observation]);
                next_observation += 1;
                run.complete_output(&observed.port_id, sequence).unwrap();
            }
            if let Some(sequence) = run.output_into(&transition.port_id, &mut event).unwrap() {
                if next_event == 0 && held_output_steps < 128 {
                    held_output_steps += 1;
                    continue;
                }
                assert_eq!(sequence, next_event as u64);
                let value = validate_canonical_structured_value(&event.encoded).unwrap();
                let octet = |name| {
                    value
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u8")
                        .unwrap()[0]
                };
                let pressed = value
                    .record_field("pressed")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap()[0]
                    != 0;
                assert_eq!(
                    (octet("usage"), pressed, octet("modifiers")),
                    expected[next_event]
                );
                next_event += 1;
                run.complete_output(&transition.port_id, sequence).unwrap();
            }
            if next_observation == 3 && next_event == 6 {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!((next_input, next_observation, next_event), (3, 3, 6));
        assert_eq!(held_output_steps, 128);
        let mut drained = false;
        for _ in 0..4096 {
            let status = run.step().unwrap();
            assert_eq!(
                run.output_into(&transition.port_id, &mut event).unwrap(),
                None
            );
            if status == KernelCompositeStatus::Complete {
                drained = true;
                break;
            }
        }
        assert!(
            drained,
            "normal frame EOF must append finish after all reports"
        );
        assert_eq!(
            run.output_terminal_into(&transition.port_id, &mut event)
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
        assert_eq!(
            run.output_terminal_into(&observed.port_id, &mut observation)
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}
