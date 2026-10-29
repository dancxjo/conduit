//! CPU proof of the production await-observation seam, without an SDK/radio fixture.
#[allow(dead_code)]
#[path = "../../../../semantics/net/src/station_attempt.rs"]
mod attempt;
#[path = "../src/inspection.rs"]
mod inspection;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use inspection::{observed_wait, StationProgress, StationSubstage as S};
fn poll<T>(work: core::pin::Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    work.poll(&mut Context::from_waker(Waker::noop()))
}
#[test]
fn cancellation_preserves_each_actual_await_stage_and_original_start() {
    let mut progress = StationProgress::new();
    for (i, stage) in [
        S::Withdrawing,
        S::Associating,
        S::WaitingAddress,
        S::StartingServices,
    ]
    .into_iter()
    .enumerate()
    {
        {
            let mut work = core::pin::pin!(observed_wait(
                &mut progress,
                stage,
                100 + i as u64,
                core::future::pending::<()>()
            ));
            assert_eq!(poll(work.as_mut()), Poll::Pending);
        }
        assert_eq!(progress.stage, stage);
        assert_eq!(progress.attempt_started_ms, Some(100));
        assert_eq!(progress.stage_started_ms, 100 + i as u64);
    }
    assert_eq!(progress.stage, S::StartingServices);
    assert_eq!(progress.attempt_started_ms, Some(100));
    assert_eq!(progress.stage_started_ms, 103);
    progress.capture(115, true, false);
    assert!(progress.link_up);
    assert!(!progress.address_up);
    assert!(!progress.services_ready);
    assert_eq!(progress.observed_at_ms, 115);
    // Cleanup cannot replace the terminal snapshot with restored AP state.
    progress.capture(200, false, false);
    assert_eq!(progress.observed_at_ms, 115);
    assert!(progress.link_up);
}
#[test]
fn observed_stages_do_not_restart_absolute_deadline_or_poll_after_expiry() {
    use core::cell::Cell;
    let clock = Cell::new(10);
    let effects = Cell::new(0);
    let mut progress = StationProgress::new();
    let mut work = core::pin::pin!(attempt::guard_operation_deadline(
        async {
            observed_wait(&mut progress, S::Associating, clock.get(), async {
                effects.set(effects.get() + 1);
                clock.set(15);
            })
            .await;
            attempt::guard_operation_deadline(
                observed_wait(&mut progress, S::WaitingAddress, clock.get(), async {
                    effects.set(effects.get() + 1);
                }),
                || clock.get() >= 15,
            )
            .await
        },
        || clock.get() >= 15
    ));
    assert_eq!(
        poll(work.as_mut()),
        Poll::Ready(Err(attempt::DeadlineExpired))
    );
    assert_eq!(effects.get(), 1);
}
#[test]
fn completed_wait_keeps_observations_distinct_from_service_authority() {
    let mut progress = StationProgress::new();
    {
        let mut work = core::pin::pin!(observed_wait(
            &mut progress,
            S::StartingServices,
            30,
            async { 7 }
        ));
        assert_eq!(poll(work.as_mut()), Poll::Ready(7));
    }
    assert!(!progress.services_ready);
    progress.capture(31, true, true);
    assert_eq!(progress.stage, S::StartingServices);
    progress.services_ready = true;
    progress.enter(S::Ready, 32);
    assert_eq!(progress.attempt_started_ms, Some(30));
}
