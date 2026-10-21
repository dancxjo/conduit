use super::*;
use crate::arch::xhci_mechanism::XhciReady;

fn transfer(slot: u8, endpoint: u8, pointer: u64) -> Event {
    Event {
        event_data: false,
        event_type: 32,
        completion_code: 1,
        slot,
        endpoint,
        residual: 0,
        pointer,
    }
}

#[test]
fn interleaved_keyboard_window_is_retained_for_its_original_owner() {
    let mut inbox = RetainedEvents::new();
    let pointer = |event: Event| event.slot == 2 && event.endpoint == 3;
    for index in 0..8 {
        let keyboard = transfer(1, 3, 0x1000 + index * 16);
        assert_eq!(inbox.poll_with(pointer, || Some(keyboard)), Ok(None));
    }
    let mouse = transfer(2, 3, 0x2000);
    assert_eq!(inbox.poll_with(pointer, || Some(mouse)), Ok(Some(mouse)));
    for index in 0..8 {
        assert_eq!(inbox.pop(), Some(transfer(1, 3, 0x1000 + index * 16)));
    }
    assert_eq!(inbox.pop(), None);
}

#[test]
fn endpoint_selection_never_skips_an_older_wrong_trb_for_a_later_match() {
    let mut inbox = RetainedEvents::new();
    for event in [
        transfer(1, 3, 0x1000),
        transfer(2, 3, 0xdead),
        transfer(1, 3, 0x1010),
        transfer(2, 3, 0x2000),
    ] {
        assert_eq!(inbox.poll_with(|_| false, || Some(event)), Ok(None));
    }
    assert_eq!(
        inbox.poll_with(
            |e| e.slot == 2,
            || panic!("retained result must not poll hardware")
        ),
        Ok(Some(transfer(2, 3, 0xdead)))
    );
    assert_eq!(
        inbox.poll_with(|e| e.slot == 2, || None),
        Ok(Some(transfer(2, 3, 0x2000)))
    );
    assert_eq!(inbox.pop(), Some(transfer(1, 3, 0x1000)));
    assert_eq!(inbox.pop(), Some(transfer(1, 3, 0x1010)));
}

#[test]
fn pressure_preserves_every_retained_event_without_consuming_hardware() {
    let mut inbox = RetainedEvents::new();
    assert_eq!(RETAINED_EVENT_SLOTS, 16);
    for index in 0..RETAINED_EVENT_SLOTS {
        assert_eq!(
            inbox.poll_with(|_| false, || Some(transfer(1, 3, index as u64 * 16))),
            Ok(None)
        );
    }
    assert_eq!(
        inbox.poll_with(|_| false, || panic!("no retention reservation")),
        Err(XhciError::EventRetentionPressure)
    );
    // A current owner may still drain its queued completion under pressure.
    assert_eq!(
        inbox.poll_with(|e| e.slot == 1, || panic!("already retained")),
        Ok(Some(transfer(1, 3, 0)))
    );
    for index in 1..RETAINED_EVENT_SLOTS {
        assert_eq!(inbox.pop(), Some(transfer(1, 3, index as u64 * 16)));
    }
    assert_eq!(inbox.pop(), None);
}

#[test]
fn no_event_and_repeated_selective_reads_preserve_foreign_completion() {
    let mut inbox = RetainedEvents::new();
    let keyboard = transfer(1, 3, 0x1230);
    assert_eq!(inbox.poll_with(|_| false, || Some(keyboard)), Ok(None));
    for _ in 0..32 {
        assert_eq!(inbox.poll_with(|e| e.slot == 2, || None), Ok(None));
    }
    assert_eq!(inbox.pop(), Some(keyboard));
    assert_eq!(inbox.pop(), None);
    assert!(core::mem::size_of::<RetainedEvents>() <= 512);
    assert!(core::mem::size_of::<XhciReady>() <= 1024);
}
