use super::*;
use conduit_core::encode_monotonic_duration;
use conduit_kernel::{HostCallOutcome, ValueStorage};

fn operation(
    store: &mut conduit_kernel::HostedValueStore,
    maximum_transitions: u64,
) -> TimedButtonAttemptBack {
    let durations = (0..maximum_transitions)
        .map(|_| store.store(&encode_monotonic_duration(50)).unwrap())
        .collect();
    TimedButtonAttemptBack::from_prepared_durations(
        durations,
        maximum_transitions,
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    )
}

fn input(operation: &mut TimedButtonAttemptBack, value: ValueRef) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [Some(value)],
        [false],
        [Some(
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )],
        None,
        8,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

fn completion(
    operation: &mut TimedButtonAttemptBack,
    request: RequestId,
    outcome: HostCallOutcome,
    bytes: Option<&[u8]>,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )],
        Some((request, outcome)),
        8,
    );
    let result = operation.step(&mut io, &StepInputBytes::test_frame([None], bytes));
    (result, io)
}

fn completed(output: Option<BoundedValueRef>) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output,
        failure: None,
    }
}

#[test]
fn fired_deadline_is_a_distinct_timeout_failure() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let transition = store.store(b"transition").unwrap();
    let marker = store.store(&[0]).unwrap();
    let mut operation = operation(&mut store, 2);
    let (outcome, io) = input(&mut operation, transition);
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_host_request().unwrap().1, HostCallId(1));
    let (outcome, io) = completion(
        &mut operation,
        RequestId(0),
        completed(Some(BoundedValueRef::new(marker, 1).unwrap())),
        Some(&[0]),
    );
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_host_request().unwrap().1, HostCallId(0));
    assert_eq!(
        completion(&mut operation, RequestId(1), completed(None), None,).0,
        attempt_step_fail(FailureCode::HostCallFailed, 4)
    );
}

#[test]
fn closed_input_before_the_required_presses_is_not_timeout_or_exhaustion() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let mut operation = operation(&mut store, 2);
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        attempt_step_fail(FailureCode::InvalidInput, 2)
    );
}

#[test]
fn total_transition_exhaustion_is_not_timeout_or_malformed_input() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 1024, 4096).unwrap();
    let first = store.store(b"released-1").unwrap();
    let second = store.store(b"released-2").unwrap();
    let mut operation = operation(&mut store, 1);
    assert_eq!(input(&mut operation, first).0, StepOutcome::Progress);
    assert_eq!(
        completion(&mut operation, RequestId(0), completed(None), None).0,
        StepOutcome::Progress
    );
    assert_eq!(
        input(&mut operation, second).0,
        attempt_step_fail(FailureCode::StorageExhausted, 1)
    );
}

#[test]
fn shared_step_cancels_the_exact_deadline_before_consuming_the_next_transition() {
    let mut store = conduit_kernel::HostedValueStore::new(12, 1024, 4096).unwrap();
    let first = store.store(b"first").unwrap();
    let next = store.store(b"next").unwrap();
    let marker = store.store(&[0]).unwrap();
    let mut operation = operation(&mut store, 3);
    assert!(StepBack::<1>::accepts_input_while_host_call_pending(
        &operation
    ));
    assert_eq!(input(&mut operation, first).0, StepOutcome::Progress);
    let (_, deadline) = completion(
        &mut operation,
        RequestId(0),
        completed(Some(BoundedValueRef::new(marker, 1).unwrap())),
        Some(&[0]),
    );
    assert_eq!(deadline.test_host_request().unwrap().0, RequestId(1));

    let (outcome, cancellation) = input(&mut operation, next);
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(cancellation.test_host_cancellation(), Some(RequestId(1)));
    assert!(!cancellation.test_consumed(PortId(0)));
    assert_eq!(
        completion(
            &mut operation,
            RequestId(1),
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
            None,
        )
        .0,
        StepOutcome::Progress
    );
    let (_, observation) = input(&mut operation, next);
    assert_eq!(observation.test_host_request().unwrap().0, RequestId(2));
    let (_, deadline) = completion(&mut operation, RequestId(2), completed(None), None);
    assert_eq!(deadline.test_host_request().unwrap().0, RequestId(3));
    assert_eq!(
        completion(&mut operation, RequestId(3), completed(None), None,).0,
        attempt_step_fail(FailureCode::HostCallFailed, 4)
    );
}

#[test]
fn observation_request_uses_the_callers_admitted_input_bound() {
    let mut store = conduit_kernel::HostedValueStore::new(8, 4096, 32768).unwrap();
    let value = store.store(b"transition").unwrap();
    let mut operation = TimedButtonAttemptBack::from_prepared_durations(Vec::new(), 2, 4096);
    let (_, io) = input(&mut operation, value);
    let request = io.test_host_request().unwrap();
    assert_eq!(request.2.admitted_bytes, 4096);
    assert_eq!(request.2.value, value);
}
