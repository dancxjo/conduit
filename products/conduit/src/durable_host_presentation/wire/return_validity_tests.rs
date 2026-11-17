use super::*;
use std::cell::RefCell;

fn scripted(
    issued: u64,
    expires: u64,
    maximum_ttl: u64,
    samples: &[(u64, u64)],
    budget_millis: u64,
) -> (Result<u64, NotBeforeRefusal>, Vec<Duration>) {
    let start = Instant::now();
    let mut samples = samples.iter();
    let waits = RefCell::new(Vec::new());
    let result = wait_until_valid(
        issued,
        expires,
        maximum_ttl,
        start + Duration::from_millis(budget_millis),
        || {
            let (unix_millis, elapsed) = samples.next().expect("bounded clock script");
            Ok(ClockSample {
                unix_millis: *unix_millis,
                monotonic: start + Duration::from_millis(*elapsed),
            })
        },
        |duration| {
            waits.borrow_mut().push(duration);
            Ok(())
        },
    );
    (result, waits.into_inner())
}

#[test]
fn already_valid_challenge_does_not_wait_or_change_local_time() {
    assert_eq!(
        scripted(1_000, 2_000, 1_000, &[(1_042, 0)], 3_000),
        (Ok(1_042), vec![])
    );
    assert_eq!(
        scripted(1_000, 2_000, 1_000, &[(1_000, 0)], 3_000),
        (Ok(1_000), vec![])
    );
}

#[test]
fn future_challenge_waits_exact_42_milliseconds_then_uses_fresh_local_time() {
    assert_eq!(
        scripted(1_042, 2_042, 1_000, &[(1_000, 0), (1_043, 43)], 3_000),
        (Ok(1_043), vec![Duration::from_millis(42)]),
    );
}

#[test]
fn expired_or_invalid_intervals_refuse_without_waiting() {
    assert_eq!(
        scripted(1_000, 2_000, 1_000, &[(2_000, 0)], 3_000),
        (Err(NotBeforeRefusal::Expired), vec![])
    );
    for (issued, expires, maximum) in [
        (2_000, 1_000, 1_000),
        (1_000, 1_000, 1_000),
        (1_000, 2_001, 1_000),
        (1_000, 2_000, 0),
    ] {
        assert_eq!(
            scripted(issued, expires, maximum, &[], 3_000),
            (Err(NotBeforeRefusal::InvalidInterval), vec![])
        );
    }
}

#[test]
fn expiry_reached_during_wait_never_returns_signing_readiness() {
    assert_eq!(
        scripted(1_042, 1_043, 1_000, &[(1_000, 0), (1_043, 43)], 3_000),
        (
            Err(NotBeforeRefusal::Expired),
            vec![Duration::from_millis(42)]
        ),
    );
}

#[test]
fn far_future_and_original_deadline_refuse_without_waiting() {
    assert_eq!(
        scripted(4_001, 5_001, 1_000, &[(1_000, 0)], 3_000),
        (Err(NotBeforeRefusal::DeadlineExceeded), vec![])
    );
    assert_eq!(
        scripted(4_000, 5_000, 1_000, &[(1_000, 0)], 3_000),
        (Err(NotBeforeRefusal::DeadlineExceeded), vec![])
    );
    assert_eq!(
        scripted(1_000, 2_000, 1_000, &[(1_000, 3_000)], 3_000),
        (Err(NotBeforeRefusal::DeadlineExceeded), vec![])
    );
}

#[test]
fn wall_clock_rollback_cannot_extend_the_original_deadline() {
    assert_eq!(
        scripted(
            1_042,
            2_042,
            1_000,
            &[(1_000, 0), (990, 43), (980, 96)],
            100
        ),
        (
            Err(NotBeforeRefusal::DeadlineExceeded),
            vec![Duration::from_millis(42), Duration::from_millis(52)]
        ),
    );
    assert_eq!(
        scripted(1_042, 2_042, 1_000, &[(1_000, 0), (1_100, 100)], 100),
        (
            Err(NotBeforeRefusal::DeadlineExceeded),
            vec![Duration::from_millis(42)]
        ),
    );
}

#[test]
fn cancellation_and_clock_failure_cannot_reach_signing_readiness() {
    let start = Instant::now();
    let mut count = 0;
    assert_eq!(
        wait_until_valid(
            1_042,
            2_042,
            1_000,
            start + Duration::from_secs(3),
            || {
                count += 1;
                Ok(ClockSample {
                    unix_millis: 1_000,
                    monotonic: start,
                })
            },
            |_| Err(NotBeforeRefusal::Cancelled)
        ),
        Err(NotBeforeRefusal::Cancelled)
    );
    assert_eq!(count, 1);
    assert_eq!(
        wait_until_valid(
            1_042,
            2_042,
            1_000,
            start + Duration::from_secs(3),
            || Err(NotBeforeRefusal::ClockUnavailable),
            |_| panic!("clock failure must not wait")
        ),
        Err(NotBeforeRefusal::ClockUnavailable)
    );
}
