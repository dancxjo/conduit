//! Synthetic DMA/controller conformance, not installed input or device evidence.
use super::super::fixture::{pointer_device, pointer_ready};
use super::*;
use crate::arch::xhci_mechanism::{synthetic_transfer_event, with_synthetic_stop_controller};

fn event(ready: HidPointerReady, index: usize) -> Event {
    Event {
        event_type: 32,
        completion_code: 1,
        slot: 2,
        endpoint: 3,
        residual: 0,
        pointer: ready.dma_physical
            + core::mem::offset_of!(PointerDma, transfer_ring) as u64
            + (index % POINTER_TRANSFER_REPORT_SLOTS * 16) as u64,
    }
}

#[test]
fn asynchronous_receive_conserves_peer_completion_and_commits_one_report() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        session.begin_followup(&mut controller, &device).unwrap();
        assert_eq!(
            session.begin_followup(&mut controller, &device),
            Err(HidPointerError::TransferOverflow)
        );
        assert!(session.followup_pending());
        assert_eq!(session.physical_transfers(), 0);
        let keyboard = Event {
            slot: 1,
            pointer: 0xfeed0,
            ..event(ready, 0)
        };
        unsafe {
            synthetic_transfer_event(&controller, keyboard, None);
        }
        assert_eq!(session.poll_followup(&mut controller, &device), Ok(None));
        assert_eq!(session.position(), (500_000, 500_000));
        unsafe {
            POINTER_DMA.reports[0] = [1, 5, (-7_i8) as u8, 0];
            synthetic_transfer_event(&controller, event(ready, 0), None);
        }
        let sample = session
            .poll_followup(&mut controller, &device)
            .unwrap()
            .unwrap();
        assert_eq!((sample.position_x, sample.position_y), (520_000, 472_000));
        assert!(sample.primary_pressed);
        assert_eq!(sample.sequence, 1);
        assert!(!session.followup_pending());
        assert_eq!(session.physical_transfers(), 1);
        assert_eq!(controller.poll_transfer_event(1, 3), Ok(Some(keyboard)));
        session.begin_followup(&mut controller, &device).unwrap();
        unsafe {
            POINTER_DMA.reports[1] = [0, 0, 0, 0];
            synthetic_transfer_event(&controller, event(ready, 1), None);
        }
        let release = session
            .poll_followup(&mut controller, &device)
            .unwrap()
            .unwrap();
        assert!(!release.primary_pressed);
        assert_eq!(release.sequence, 2);
        assert_eq!(session.physical_transfers(), 2);
    });
}

#[test]
fn stale_attachment_and_wrong_trb_leave_original_pending_state_fenced() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        session.begin_followup(&mut controller, &device).unwrap();
        unsafe {
            synthetic_transfer_event(&controller, event(ready, 1), None);
        }
        device.attachment_epoch += 1;
        assert_eq!(
            session.poll_followup(&mut controller, &device),
            Err(HidPointerError::StaleAttachment)
        );
        device.attachment_epoch -= 1;
        assert_eq!(
            session.poll_followup(&mut controller, &device),
            Err(HidPointerError::WrongCompletion)
        );
        assert!(session.followup_pending());
        assert_eq!(session.sequence(), 0);
        assert_eq!(session.position(), (500_000, 500_000));
        assert_eq!(
            session.begin_followup(&mut controller, &device),
            Err(HidPointerError::TransferOverflow)
        );
    });
}

#[test]
fn retention_pressure_is_distinct_and_does_not_replace_pending_pointer_report() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        session.begin_followup(&mut controller, &device).unwrap();
        for index in 0..16 {
            unsafe {
                synthetic_transfer_event(
                    &controller,
                    Event {
                        slot: 1,
                        pointer: index * 16,
                        ..event(ready, 0)
                    },
                    None,
                );
            }
            assert_eq!(session.poll_followup(&mut controller, &device), Ok(None));
        }
        assert_eq!(
            session.poll_followup(&mut controller, &device),
            Err(HidPointerError::CompletionPressure)
        );
        assert_eq!(session.sequence(), 0);
        assert!(session.followup_pending());
        for index in 0..16 {
            assert_eq!(
                controller
                    .poll_transfer_event(1, 3)
                    .unwrap()
                    .unwrap()
                    .pointer,
                index * 16
            );
        }
    });
}

#[test]
fn existing_exclusive_receiver_uses_the_same_exact_pending_completion() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        unsafe {
            POINTER_DMA.reports[0] = [0, 1, 2, 0];
            synthetic_transfer_event(&controller, event(ready, 0), None);
        }
        let sample = session.receive(&mut controller, &device).unwrap();
        assert_eq!((sample.position_x, sample.position_y), (504_000, 508_000));
        assert_eq!(sample.sequence, 1);
        assert!(!session.followup_pending());
    });
}

#[test]
fn unrelated_port_observation_is_retained_and_actual_attachment_loss_is_distinct() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        session.begin_followup(&mut controller, &device).unwrap();
        let other_port = Event {
            event_type: 34,
            slot: 0,
            endpoint: 0,
            pointer: 2 << 24,
            ..event(ready, 0)
        };
        unsafe {
            synthetic_transfer_event(&controller, other_port, None);
        }
        assert_eq!(session.poll_followup(&mut controller, &device), Ok(None));
        controller.write_port_status(1, 0);
        assert_eq!(
            session.poll_followup(&mut controller, &device),
            Err(HidPointerError::DeviceRemoved)
        );
        assert!(session.followup_pending());
        assert_eq!(session.sequence(), 0);
        assert_eq!(controller.poll_event(), Some(other_port));
    });
}

#[test]
fn malformed_completed_report_cannot_commit_partial_motion_or_rearm_itself() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        let mut device = pointer_device();
        device.root_port = 1;
        let ready = pointer_ready(&device);
        let mut session = start_pointer_session(ready);
        session.begin_followup(&mut controller, &device).unwrap();
        unsafe {
            POINTER_DMA.reports[0] = [0x80, 127, 127, 0];
            synthetic_transfer_event(&controller, event(ready, 0), None);
        }
        assert_eq!(
            session.poll_followup(&mut controller, &device),
            Err(HidPointerError::ReservedButtons)
        );
        assert!(session.followup_pending());
        assert_eq!(session.sequence(), 0);
        assert_eq!(session.physical_transfers(), 0);
        assert_eq!(session.position(), (500_000, 500_000));
        assert_eq!(
            session.begin_followup(&mut controller, &device),
            Err(HidPointerError::TransferOverflow)
        );
    });
}
