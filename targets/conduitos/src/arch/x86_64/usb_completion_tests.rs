use super::*;

fn event(pointer: u64, code: u8, residual: u32) -> Event {
    Event {
        event_type: 32,
        completion_code: code,
        slot: 1,
        endpoint: 1,
        residual,
        pointer,
    }
}

fn pending(input: bool) -> ControlCompletion {
    ControlCompletion::new(1, 16, Some(32), 48, 8, input)
}

#[test]
fn short_data_waits_for_status_and_retains_its_residual() {
    let mut call = pending(true);
    assert!(call.observe(event(32, 13, 5)).unwrap().is_none());
    let mut port = event(0, 1, 0);
    port.event_type = 34;
    assert!(call.observe(port).unwrap().is_none());
    let result = call.observe(event(48, 1, 0)).unwrap().unwrap();
    assert_eq!(result.bytes, 3);
    assert!(result.short);
    assert!(matches!(
        call.observe(event(48, 1, 0)),
        Err(UsbError::MalformedCompletion)
    ));
}

#[test]
fn full_and_no_data_operations_finish_only_on_status() {
    let result = pending(true).observe(event(48, 1, 0)).unwrap().unwrap();
    assert_eq!(result.bytes, 8);
    assert!(!result.short);
    let mut call = ControlCompletion::new(1, 16, None, 32, 0, false);
    let result = call.observe(event(32, 1, 0)).unwrap().unwrap();
    assert_eq!(result.bytes, 0);
    assert!(!result.short);
}

#[test]
fn malformed_stage_counts_and_duplicate_short_events_are_not_success() {
    for malformed in [
        event(32, 13, 9),
        event(32, 13, 65536),
        event(48, 1, 1),
        event(48, 13, 0),
        event(16, 1, 0),
        event(32, 1, 0),
    ] {
        assert!(matches!(
            pending(true).observe(malformed),
            Err(UsbError::MalformedCompletion)
        ));
    }
    assert!(matches!(
        pending(false).observe(event(32, 13, 1)),
        Err(UsbError::MalformedCompletion)
    ));
    let mut call = pending(true);
    assert!(call.observe(event(32, 13, 1)).unwrap().is_none());
    assert!(matches!(
        call.observe(event(32, 13, 1)),
        Err(UsbError::MalformedCompletion)
    ));
}

#[test]
fn foreign_events_and_real_endpoint_failures_stay_distinct() {
    for (observed, expected) in [
        (
            Event {
                slot: 2,
                ..event(48, 1, 0)
            },
            UsbError::WrongSlot,
        ),
        (
            Event {
                endpoint: 2,
                ..event(48, 1, 0)
            },
            UsbError::WrongEndpoint,
        ),
        (event(64, 1, 0), UsbError::WrongController),
        (event(32, 6, 0), UsbError::ControlStall),
        (event(16, 4, 0), UsbError::ControlError),
    ] {
        assert!(matches!(pending(true).observe(observed), Err(reason) if reason == expected));
    }
}
