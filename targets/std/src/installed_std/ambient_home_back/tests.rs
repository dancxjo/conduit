//! Step-protocol conformance with N's real gate, not a route or deployment proof.
use super::*;
use conduit_kernel::HostCallOutcome;

fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
fn frame(completion: Option<u32>) -> StepIo<1> {
    StepIo::test_frame(
        [Some(value())],
        [true],
        [Some(INPUT_BYTES)],
        completion.map(|id| {
            (
                RequestId(id),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
        }),
        32,
    )
}
fn step(back: &mut AmbientHomeBack, io: &mut StepIo<1>) -> StepOutcome {
    back.step(io, &StepInputBytes::test_frame([None], None))
}
fn commit(back: &mut AmbientHomeBack) {
    <AmbientHomeBack as StepBack<1>>::step_committed(back);
}
fn start(back: &mut AmbientHomeBack) {
    let mut io = frame(None);
    assert_eq!(step(back, &mut io), StepOutcome::Progress);
    assert!(io.test_retained(INPUT));
    assert!(!io.test_consumed(INPUT));
    commit(back);
}

#[test]
fn input_is_retained_through_projection_and_observer_pressure() {
    let mut back = AmbientHomeBack::prepare("home/test".into(), 2).unwrap();
    drop(back.observation());
    assert!(back.retained_backing_bytes().unwrap() > 0);
    start(&mut back);
    let mut projected = frame(Some(0));
    assert_eq!(step(&mut back, &mut projected), StepOutcome::Progress);
    assert!(projected.test_retained(INPUT));
    assert!(!projected.test_consumed(INPUT));
    commit(&mut back);
    for _ in 0..8 {
        let mut pressure = frame(None);
        assert_eq!(step(&mut back, &mut pressure), StepOutcome::Await);
        assert!(!pressure.test_consumed(INPUT));
        assert_eq!(back.accepted_updates, 0);
    }
    let mut accepted = frame(Some(1));
    assert_eq!(step(&mut back, &mut accepted), StepOutcome::Progress);
    assert!(accepted.test_consumed(INPUT));
    assert_eq!(back.accepted_updates, 0);
    commit(&mut back);
    assert_eq!(back.accepted_updates, 1);
    assert!(back.pending.is_none());
    let mut closed = StepIo::test_frame([None], [true], [Some(INPUT_BYTES)], None, 32);
    assert_eq!(step(&mut back, &mut closed), StepOutcome::Complete);
    assert!(closed.test_consumed_closed(INPUT));
    commit(&mut back);
}

#[test]
fn a_step_without_commit_does_not_advance_either_phase() {
    let mut back = AmbientHomeBack::prepare("home/test".into(), 2).unwrap();
    let mut uncommitted = frame(None);
    assert_eq!(step(&mut back, &mut uncommitted), StepOutcome::Progress);
    assert!(back.pending.is_none());
    start(&mut back);
    let mut projection = frame(Some(0));
    assert_eq!(step(&mut back, &mut projection), StepOutcome::Progress);
    assert!(!back.pending.unwrap().custody);
    let mut retry = frame(Some(0));
    assert_eq!(step(&mut back, &mut retry), StepOutcome::Progress);
    commit(&mut back);
    assert!(back.pending.unwrap().custody);
    let mut accepted = frame(Some(1));
    assert_eq!(step(&mut back, &mut accepted), StepOutcome::Progress);
    assert_eq!(back.accepted_updates, 0);
    let mut retry = frame(Some(1));
    assert_eq!(step(&mut back, &mut retry), StepOutcome::Progress);
    commit(&mut back);
    assert_eq!(back.accepted_updates, 1);
}

#[test]
fn unsolicited_wrong_and_late_completions_never_consume_input() {
    let mut back = AmbientHomeBack::prepare("home/test".into(), 2).unwrap();
    let mut unsolicited = frame(Some(0));
    assert!(matches!(
        step(&mut back, &mut unsolicited),
        StepOutcome::Fail(_)
    ));
    assert!(!unsolicited.test_consumed(INPUT));
    start(&mut back);
    let mut wrong = frame(Some(77));
    assert!(matches!(step(&mut back, &mut wrong), StepOutcome::Fail(_)));
    assert!(!wrong.test_consumed(INPUT));
    <AmbientHomeBack as StepBack<1>>::cancel(&mut back);
    let mut late = frame(Some(0));
    assert!(matches!(step(&mut back, &mut late), StepOutcome::Fail(_)));
    assert!(!late.test_consumed(INPUT));
}

#[test]
fn replaced_payload_or_failed_projection_cannot_reach_custody() {
    let mut back = AmbientHomeBack::prepare("home/test".into(), 2).unwrap();
    start(&mut back);
    let mut different = StepIo::test_frame(
        [Some(ValueRef {
            generation: 2,
            ..value()
        })],
        [true],
        [Some(INPUT_BYTES)],
        None,
        32,
    );
    assert!(matches!(
        step(&mut back, &mut different),
        StepOutcome::Fail(_)
    ));
    let mut failed_call = StepIo::test_frame(
        [Some(value())],
        [true],
        [Some(INPUT_BYTES)],
        Some((
            RequestId(0),
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 1,
                }),
            },
        )),
        32,
    );
    assert!(matches!(
        step(&mut back, &mut failed_call),
        StepOutcome::Fail(_)
    ));
    assert!(!failed_call.test_consumed(INPUT));
    assert!(!back.pending.unwrap().custody);
}

#[test]
fn finite_update_scope_and_request_identity_arithmetic_are_explicit() {
    for maximum in [0, u32::MAX] {
        assert!(AmbientHomeBack::prepare("home/test".into(), maximum).is_err());
    }
    let mut back = AmbientHomeBack::prepare("home/test".into(), 1).unwrap();
    start(&mut back);
    let mut projected = frame(Some(0));
    assert_eq!(step(&mut back, &mut projected), StepOutcome::Progress);
    commit(&mut back);
    let mut accepted = frame(Some(1));
    assert_eq!(step(&mut back, &mut accepted), StepOutcome::Progress);
    commit(&mut back);
    let mut excess = frame(None);
    assert!(matches!(
        step(&mut back, &mut excess),
        StepOutcome::Fail(Failure {
            code: FailureCode::StorageExhausted,
            ..
        })
    ));
    assert!(!excess.test_consumed(INPUT));
}

#[test]
fn oversized_input_is_rejected_before_borrow_or_consumption() {
    let mut back = AmbientHomeBack::prepare("home/test".into(), 1).unwrap();
    let mut oversized = StepIo::test_frame(
        [Some(ValueRef {
            byte_len: INPUT_BYTES + 1,
            ..value()
        })],
        [true],
        [Some(INPUT_BYTES + 1)],
        None,
        32,
    );
    assert!(matches!(
        step(&mut back, &mut oversized),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            ..
        })
    ));
    assert!(!oversized.test_retained(INPUT));
    assert!(!oversized.test_consumed(INPUT));
}

#[test]
fn resident_gate_retains_two_call_protocol_past_finite_default_and_fences_request_identity() {
    let mut back = AmbientHomeBack::prepare_resident("home/resident".into(), INPUT_BYTES).unwrap();
    let backing = back.retained_backing_bytes().unwrap();
    let probe = crate::allocation_probe::begin();
    for update in 0..1500 {
        start(&mut back);
        let mut projected = frame(Some(update * 2));
        assert_eq!(step(&mut back, &mut projected), StepOutcome::Progress);
        commit(&mut back);
        let mut accepted = frame(Some(update * 2 + 1));
        assert_eq!(step(&mut back, &mut accepted), StepOutcome::Progress);
        commit(&mut back);
    }
    assert_eq!(probe.finish_bytes(), 0);
    assert_eq!(back.retained_backing_bytes().unwrap(), backing);
    back.accepted_updates = u32::MAX / 2 + 1;
    let mut exhausted = frame(None);
    assert_eq!(
        step(&mut back, &mut exhausted),
        failed(FailureCode::IdentityCapacityExhausted)
    );
    assert!(exhausted.test_host_request().is_none());
}
