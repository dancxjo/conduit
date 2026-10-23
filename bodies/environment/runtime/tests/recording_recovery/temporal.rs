//! The temporal bridge preserves the typed value reference; it owns no JSON
//! parser, storage receipt or recovery cursor.
use super::allocation_probe;
use conduit_environment_runtime::recording::*;
use conduit_kernel::{self as k, scheduler::*};
fn offer(back: &mut RecordingSingletonBack, ready: bool, fuel: u16) -> (StepOutcome, StepIo<1>) {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 2,
    };
    let mut io = StepIo::test_frame([Some(value)], [false], [ready.then_some(4096)], None, fuel);
    let result = back.step(&mut io, &StepInputBytes::test_frame([Some(b"{}")], None));
    (result, io)
}
#[test]
fn singleton_commit_preserves_value_identity_without_allocating_or_repeating() {
    let kind = recording_singleton_kind();
    assert_eq!(kind.inputs[0].temporal, conduit_core::PortTemporal::Value);
    assert_eq!(
        kind.outputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    let mut back = RecordingSingletonBack::default();
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(offer(&mut back, false, 16).0, StepOutcome::Await);
        assert!(matches!(offer(&mut back, true, 0).0, StepOutcome::Fail(_)));
        <RecordingSingletonBack as StepBack<1>>::step_committed(&mut back);
        let (outcome, io) = offer(&mut back, true, 16);
        assert_eq!(outcome, StepOutcome::Complete);
        assert!(io.test_consumed(k::PortId(0)));
        assert_eq!(
            io.test_output(k::PortId(0)),
            Some(k::ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 2
            })
        );
        // Discarding an uncommitted completion does not retire the bridge.
        assert_eq!(offer(&mut back, true, 16).0, StepOutcome::Complete);
        <RecordingSingletonBack as StepBack<1>>::step_committed(&mut back);
        assert!(matches!(
            offer(&mut back, true, 16).0,
            StepOutcome::Fail(k::Failure {
                code: k::FailureCode::InvalidLifecycle,
                ..
            })
        ));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn cancelled_or_missing_single_value_is_not_an_empty_successful_flow() {
    let mut back = RecordingSingletonBack::default();
    <RecordingSingletonBack as StepBack<1>>::cancel(&mut back);
    assert!(matches!(
        offer(&mut back, true, 16).0,
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::Cancelled,
            ..
        })
    ));
    let mut back = RecordingSingletonBack::default();
    assert!(matches!(
        back.step(
            &mut StepIo::test_frame([None], [true], [Some(4096)], None, 16),
            &StepInputBytes::test_frame([None], None)
        ),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::InvalidInput,
            ..
        })
    ));
}
