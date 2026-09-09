use super::*;
use conduit_human::{KeyModifiers, KeyTransition};

fn idle(kernel: &mut KeyboardTextKernel) -> (HostOperationRequest, Option<PresentationFragment>) {
    let mut pending = None;
    let mut result = None;
    for _ in 0..256 {
        while let Some(request) = kernel.next_host_request() {
            match kernel.request_kind(request).unwrap() {
                KeyboardTextRequestKind::Keyboard => {
                    assert!(pending.replace(request).is_none());
                }
                KeyboardTextRequestKind::Keymap => kernel.complete_keymap(request).unwrap(),
                KeyboardTextRequestKind::Upper => kernel.complete_upper(request).unwrap(),
                KeyboardTextRequestKind::Presentation => {
                    assert!(
                        result
                            .replace(kernel.complete_presentation(request).unwrap())
                            .is_none()
                    );
                }
            }
        }
        match kernel.step().unwrap() {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => return (pending.expect("standing keyboard request"), result),
            status => panic!("standing Play unexpectedly settled: {status:?}"),
        }
    }
    panic!("input failed to reach quiescence within admitted work bound");
}

#[test]
fn thousands_of_keys_reuse_values_and_retain_a_truthful_sign_gap_in_one_kernel() {
    let prepared = crate::keyboard_text_play_tests::prepared();
    let mut kernel = KeyboardTextKernel::prepare_standing(&prepared).unwrap();
    let (mut pending, first) = idle(&mut kernel);
    assert!(first.is_none());
    let items = kernel.scheduler.values().used_items();
    let bytes = kernel.scheduler.values().used_bytes();
    for index in 0..2_048 {
        let pressed = index % 2 == 0;
        let event = KeyEvent::new(
            4,
            if pressed {
                KeyTransition::Pressed
            } else {
                KeyTransition::Released
            },
            KeyModifiers::NONE,
        )
        .unwrap();
        kernel.complete_keyboard(pending, event).unwrap();
        let (next, result) = idle(&mut kernel);
        assert_ne!(next.request, pending.request);
        assert_eq!(
            result.as_ref().map(PresentationFragment::as_bytes),
            pressed.then_some(&b"A"[..])
        );
        assert_eq!(kernel.scheduler.values().used_items(), items);
        assert_eq!(kernel.scheduler.values().used_bytes(), bytes);
        pending = next;
    }
    let gap = kernel
        .sign_retention_gap()
        .expect("bounded Sign history discloses eviction");
    assert!(gap.entries > 0 && gap.first_sequence <= gap.last_sequence);
    kernel.cancel().unwrap();
    assert_eq!(kernel.step().unwrap(), SchedulerStatus::Cancelled);
    assert_eq!(kernel.scheduler.values().used_items(), 0);
    assert_eq!(kernel.scheduler.pending_host_operation_count(), 0);
    assert!(
        kernel
            .complete_keyboard(
                pending,
                KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
            )
            .is_err()
    );
}
