use super::*;

fn published(cycle: u32) -> [u32; 4] {
    [
        0x5678_9000,
        0x1234,
        (1 << 24) | 7,
        (2 << 24) | (3 << 16) | (32 << 10) | cycle,
    ]
}

#[test]
fn publication_during_ownership_read_defers_the_whole_event_on_both_wrap_cycles() {
    for cycle in [0, 1] {
        let new = published(cycle);
        let mut dma = [0xdead_beef, 0, 0, cycle ^ 1];
        let mut reads = 0;
        // The controller publishes after the consumer's first word load.
        // A payload-first reader would retain dead_beef, then accept new control.
        let event = read_owned_event(cycle, |index| {
            let word = dma[index];
            reads += 1;
            dma = new;
            word
        });
        assert_eq!(event, None);
        assert_eq!(reads, 1, "unowned payload must not be read");
        let event = read_owned_event(cycle, |index| dma[index]).unwrap();
        assert_eq!(event.pointer, 0x1234_5678_9000);
        assert_eq!(event.event_type, 32);
        assert_eq!(event.completion_code, 1);
        assert_eq!(event.residual, 7);
        assert_eq!((event.slot, event.endpoint), (2, 3));
    }
}

#[test]
fn unowned_slot_never_exposes_previous_lap_payload() {
    for cycle in [0, 1] {
        assert_eq!(
            read_owned_event(cycle, |index| {
                assert_eq!(index, 3, "only ownership is observable before publication");
                cycle ^ 1
            }),
            None
        );
    }
}

#[test]
fn owned_slot_reads_control_before_payload_words() {
    let words = published(1);
    let mut indices = [usize::MAX; 4];
    let mut cursor = 0;
    let event = read_owned_event(1, |index| {
        indices[cursor] = index;
        cursor += 1;
        words[index]
    })
    .unwrap();
    assert_eq!(indices, [3, 0, 1, 2]);
    assert_eq!(event.pointer, 0x1234_5678_9000);
}

#[test]
fn owned_refusals_reach_the_existing_completion_validator_unchanged() {
    let mut words = published(1);
    words[2] = (6 << 24) | 3;
    words[3] = (9 << 24) | (5 << 16) | (33 << 10) | 1;
    let event = read_owned_event(1, |index| words[index]).unwrap();
    assert_eq!((event.event_type, event.completion_code), (33, 6));
    assert_eq!((event.slot, event.endpoint, event.residual), (9, 5, 3));
}

#[test]
fn controller_preserves_keyboard_completion_while_consuming_a_command_ack() {
    use crate::arch::xhci_mechanism::{
        synthetic_next_stop_ack, synthetic_transfer_event, with_synthetic_stop_controller,
    };
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let keyboard = Event {
            event_type: 32,
            completion_code: 1,
            slot: 1,
            endpoint: 3,
            residual: 0,
            pointer: 0x1230,
        };
        unsafe {
            synthetic_transfer_event(&controller, keyboard, None);
        }
        assert_eq!(controller.poll_transfer_event(2, 3), Ok(None));
        unsafe {
            synthetic_next_stop_ack(&controller);
        }
        assert_eq!(
            controller.command([0, 0, 0, 9 << 10]).unwrap().event_type,
            33
        );
        assert_eq!(controller.poll_transfer_event(1, 3), Ok(Some(keyboard)));
        assert!(controller.poll_event().is_none());
    });
}

#[test]
fn invalid_endpoint_interest_refuses_before_consuming_controller_ownership() {
    use crate::arch::xhci_mechanism::with_synthetic_stop_controller;
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        assert_eq!(
            controller.poll_transfer_event(0, 3),
            Err(XhciError::InvalidLayout)
        );
        assert_eq!(
            controller.poll_transfer_event(1, 0),
            Err(XhciError::InvalidLayout)
        );
        assert_eq!(controller.poll_event().unwrap().event_type, 33);
    });
}

#[test]
fn unexpected_command_ack_during_transfer_cannot_preack_a_later_stop() {
    use crate::arch::xhci_mechanism::{
        synthetic_stop_ack_event, synthetic_stop_dcbaa, with_synthetic_stop_controller,
    };
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        // The original unsolicited successful ack must reach the endpoint's
        // refusal, not wait in an inbox until a command happens to use its TRB.
        assert_eq!(
            controller
                .poll_transfer_event(1, 1)
                .unwrap()
                .unwrap()
                .event_type,
            33
        );
        unsafe {
            synthetic_stop_ack_event(&controller, 1, 6);
        }
        assert_eq!(
            controller.disable_removed_slot(1),
            Err(XhciError::UnexpectedCompletion)
        );
        unsafe {
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
    });
}
