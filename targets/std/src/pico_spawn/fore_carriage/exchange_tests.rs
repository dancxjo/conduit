//! Serial application delivery proof using CPU framed peers; no media ACK.
use super::*;
use std::cell::Cell;
fn ready() -> Carriage<Peer> {
    let mut c = carriage();
    c.handshake(&mut [0; 128], TIMEOUT).unwrap();
    c.frames.reply = Some(b"read-result");
    c
}
#[test]
fn consumes_one_result_before_ack_and_preserves_both_sequence_identities() {
    let mut c = ready();
    let used = c.state.used_frames;
    let called = Cell::new(0);
    assert_eq!(
        c.exchange(b"index", &mut [0; 128], TIMEOUT, |value| {
            assert_eq!(value, b"read-result");
            called.set(called.get() + 1);
            Ok(())
        }),
        Ok(ForeExchangeOutcome::Delivered)
    );
    assert_eq!(called.get(), 1);
    assert_eq!(c.state.used_frames - used, 6);
    assert_eq!(c.state.machines[0].next_sequence(), 1);
    assert_eq!(c.state.machines[1].next_sequence(), 1);
    assert_eq!(c.frames.machines[1].next_sequence(), 1);
    assert!(c.frames.queue.is_empty());
    assert!(!c.state.retired);
}
#[test]
fn pressure_never_retries_or_consumes_and_explicit_next_attempt_remains_possible() {
    let mut c = ready();
    c.frames.pressure = true;
    let writes = c.frames.writes;
    assert_eq!(
        c.exchange(b"index", &mut [0; 128], TIMEOUT, |_| panic!(
            "not delivered"
        )),
        Ok(ForeExchangeOutcome::Pressure)
    );
    assert_eq!(c.frames.writes, writes + 1);
    assert_eq!(c.state.machines[0].next_sequence(), 0);
    assert!(!c.state.retired);
    assert_eq!(
        c.exchange(b"index", &mut [0; 128], TIMEOUT, |_| Ok(())),
        Ok(ForeExchangeOutcome::Delivered)
    );
}
#[test]
fn refused_or_early_result_gets_no_result_ack_and_locally_retires() {
    for early in [false, true] {
        let mut c = ready();
        c.frames.early_reply = early;
        let writes = c.frames.writes;
        let result = c.exchange(b"index", &mut [0; 128], TIMEOUT, |_| {
            assert!(!early, "cannot consume result before input delivery");
            Err(())
        });
        assert_eq!(
            result,
            Err(if early {
                ForeCarriageError::Scope
            } else {
                ForeCarriageError::ConsumerRefused
            })
        );
        assert_eq!(c.frames.writes, writes + 1, "only request was sent");
        assert_eq!(c.frames.machines[1].next_sequence(), 0);
        assert!(c.state.retired);
    }
}
#[test]
fn known_shortages_refuse_before_request_and_preserve_session() {
    for case in 0..4 {
        let mut c = ready();
        let mut output = [0; 128];
        let bytes = [0; 17];
        if case == 0 {
            c.state.bounds.maximum_frames = c.state.used_frames + 5;
        }
        let writes = c.frames.writes;
        let result = c.exchange(
            if case == 1 { &bytes } else { b"index" },
            if case == 2 {
                &mut output[..127]
            } else {
                &mut output
            },
            if case == 3 { Duration::ZERO } else { TIMEOUT },
            |_| panic!("no IO"),
        );
        assert_eq!(result, Err(ForeCarriageError::Bounds));
        assert_eq!(c.frames.writes, writes);
        assert!(!c.state.retired);
    }
}
#[test]
fn lost_final_ack_is_uncertain_after_one_consumption_without_retry() {
    let mut c = ready();
    c.frames.fail_send_at = Some(c.frames.writes + 3);
    let called = Cell::new(0);
    assert_eq!(
        c.exchange(b"index", &mut [0; 128], TIMEOUT, |_| {
            called.set(called.get() + 1);
            Ok(())
        }),
        Err(ForeCarriageError::OutcomeUnknown)
    );
    assert_eq!(called.get(), 1);
    assert_eq!(c.frames.machines[1].next_sequence(), 0);
    assert!(c.state.retired);
}
