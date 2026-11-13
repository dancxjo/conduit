//! Real commit/retry/cancellation semantics for an immutable typed fact.
use conduit_core::{kind_id, StructuredInfoType as T, StructuredInfoValue as V};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_typed_retention::PreparedLinkedSource;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn fixture() -> (Vec<u8>, Vec<u8>) {
    let ty = T::leaf(kind_id("value/u8")).unwrap();
    let value = V::leaf(ty.clone(), vec![42])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    (ty.canonical_bytes().unwrap(), value)
}
fn frame(ready: bool, fuel: u16) -> StepIo<1> {
    StepIo::test_frame([None], [false], [ready.then_some(1024)], None, fuel)
}
#[test]
fn pressure_and_uncommitted_steps_do_not_lose_or_duplicate_the_original_fact() {
    let (schema, value) = fixture();
    let (mut source, cost) =
        allocation_probe::observe(|| PreparedLinkedSource::prepare(&schema, &value, 1024).unwrap());
    assert_eq!((cost.allocation_attempts, cost.reallocations), (0, 0));
    let input = StepInputBytes::test_frame([None], None);
    let ((), cost) = allocation_probe::observe(|| {
        assert!(
            <PreparedLinkedSource as StepBack<1>>::prepared_output(&source, PortId(0)).is_none()
        );
        assert_eq!(
            source.step(&mut frame(false, 4), &input),
            StepOutcome::Await
        );
        assert_eq!(
            source.step(&mut frame(true, 4), &input),
            StepOutcome::Progress
        );
        assert_eq!(
            <PreparedLinkedSource as StepBack<1>>::prepared_output(&source, PortId(0)),
            Some(value.as_slice())
        );
        // No commit: the exact original value remains available on retry.
        assert_eq!(
            source.step(&mut frame(false, 4), &input),
            StepOutcome::Await
        );
        assert_eq!(
            source.step(&mut frame(true, 4), &input),
            StepOutcome::Progress
        );
        assert_eq!(
            <PreparedLinkedSource as StepBack<1>>::prepared_output(&source, PortId(0))
                .unwrap()
                .as_ptr(),
            value.as_ptr()
        );
        <PreparedLinkedSource as StepBack<1>>::step_committed(&mut source);
        assert_eq!(
            source.step(&mut frame(true, 4), &input),
            StepOutcome::Complete
        );
        assert!(
            <PreparedLinkedSource as StepBack<1>>::prepared_output(&source, PortId(0)).is_none()
        );
    });
    assert_eq!((cost.allocation_attempts, cost.reallocations), (0, 0));
}
#[test]
fn cancellation_and_bad_preparation_cannot_publish_configuration() {
    let (schema, value) = fixture();
    let mut source = PreparedLinkedSource::prepare(&schema, &value, 1024).unwrap();
    let input = StepInputBytes::test_frame([None], None);
    assert_eq!(
        source.step(&mut frame(true, 4), &input),
        StepOutcome::Progress
    );
    <PreparedLinkedSource as StepBack<1>>::cancel(&mut source);
    <PreparedLinkedSource as StepBack<1>>::step_committed(&mut source);
    assert_eq!(
        source.step(&mut frame(true, 4), &input),
        StepOutcome::Fail(Failure {
            code: FailureCode::Cancelled,
            detail: 1
        })
    );
    assert!(<PreparedLinkedSource as StepBack<1>>::prepared_output(&source, PortId(0)).is_none());
    assert!(PreparedLinkedSource::prepare(&schema, &value, value.len() - 1).is_err());
    let mut bad = value.clone();
    bad.push(0);
    assert!(PreparedLinkedSource::prepare(&schema, &bad, 1024).is_err());
    let other = T::leaf(kind_id("value/u16"))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(PreparedLinkedSource::prepare(&other, &value, 1024).is_err());
}
