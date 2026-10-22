//! Deterministic future-cancellation and phase ownership, not physical timing.
use conduit_net::*;
use core::{
    future::{pending, Future},
    task::{Context, Poll, Waker},
};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Operation {
    Withdraw,
    Associate,
    Address,
    Restore,
}
struct Provider {
    operations: Vec<Operation>,
    blocked: Option<Operation>,
    refused: Option<Operation>,
    unavailable: Option<Operation>,
    active: bool,
}
impl Provider {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
            blocked: None,
            refused: None,
            unavailable: None,
            active: false,
        }
    }
    async fn perform(&mut self, operation: Operation) -> Result<(), StationFailure> {
        assert!(
            !self.active,
            "prior operation must be dropped before cleanup"
        );
        self.operations.push(operation);
        self.active = true;
        struct Active<'a>(&'a mut bool);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                *self.0 = false;
            }
        }
        let _active = Active(&mut self.active);
        if self.blocked == Some(operation) {
            pending::<()>().await;
        }
        if self.refused == Some(operation) {
            return Err(StationFailure::Rejected);
        }
        if self.unavailable == Some(operation) {
            return Err(StationFailure::ProviderUnavailable);
        }
        Ok(())
    }
}
impl StationOperations for Provider {
    async fn associate(&mut self) -> Result<(), StationFailure> {
        self.perform(Operation::Associate).await
    }
    async fn configure_address(&mut self) -> Result<(), StationFailure> {
        self.perform(Operation::Address).await
    }
}
impl AccessPointSwitchOperations for Provider {
    async fn withdraw_access_point(&mut self) -> Result<(), StationFailure> {
        self.perform(Operation::Withdraw).await
    }
    async fn restore_access_point(&mut self) -> Result<(), StationFailure> {
        self.perform(Operation::Restore).await
    }
}
struct Deadline {
    calls: usize,
}
impl OperationDeadline for Deadline {
    async fn run<T>(&mut self, work: impl Future<Output = T>) -> Result<T, DeadlineExpired> {
        self.calls += 1;
        let mut work = std::pin::pin!(work);
        let mut context = Context::from_waker(Waker::noop());
        match work.as_mut().poll(&mut context) {
            Poll::Ready(v) => Ok(v),
            Poll::Pending => Err(DeadlineExpired),
        }
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("deterministic deadline must finish"),
    }
}
#[test]
fn station_success_requires_one_budget_for_association_and_address_configuration() {
    let mut provider = Provider::new();
    let mut deadline = Deadline { calls: 0 };
    assert_eq!(
        run(associate_and_configure(&mut provider, &mut deadline)),
        Ok(())
    );
    assert_eq!(deadline.calls, 1);
    assert_eq!(
        provider.operations,
        [Operation::Associate, Operation::Address]
    );
}
#[test]
fn each_failed_or_expired_stage_restores_ap_once_after_cancelling_inflight_work() {
    for (operation, phase) in [
        (Operation::Withdraw, StationPhase::WithdrawAccessPoint),
        (Operation::Associate, StationPhase::Associate),
        (Operation::Address, StationPhase::ConfigureAddress),
    ] {
        for timeout in [false, true] {
            let mut provider = Provider::new();
            if timeout {
                provider.blocked = Some(operation);
            } else {
                provider.refused = Some(operation);
            }
            let mut attempt = Deadline { calls: 0 };
            let mut restore = Deadline { calls: 0 };
            assert_eq!(
                run(switch_from_access_point(
                    &mut provider,
                    &mut attempt,
                    &mut restore
                )),
                AccessPointSwitchOutcome::AccessPointRestored {
                    failure: StationAttemptFailure {
                        phase,
                        reason: if timeout {
                            StationFailure::Deadline
                        } else {
                            StationFailure::Rejected
                        },
                    }
                }
            );
            assert_eq!((attempt.calls, restore.calls), (1, 1));
            assert_eq!(provider.operations.last(), Some(&Operation::Restore));
            assert!(!provider.active);
        }
    }
}
#[test]
fn failed_restoration_is_unavailable_and_never_a_claim_of_ap_readiness() {
    for timeout in [false, true] {
        let mut provider = Provider::new();
        provider.refused = Some(Operation::Associate);
        if timeout {
            provider.blocked = Some(Operation::Restore);
        } else {
            provider.unavailable = Some(Operation::Restore);
        }
        let mut attempt = Deadline { calls: 0 };
        let mut restore = Deadline { calls: 0 };
        assert_eq!(
            run(switch_from_access_point(
                &mut provider,
                &mut attempt,
                &mut restore
            )),
            AccessPointSwitchOutcome::Unavailable {
                failure: StationAttemptFailure {
                    phase: StationPhase::Associate,
                    reason: StationFailure::Rejected
                },
                restoration: if timeout {
                    StationFailure::Deadline
                } else {
                    StationFailure::ProviderUnavailable
                },
            }
        );
        assert_eq!((attempt.calls, restore.calls), (1, 1));
        assert!(!provider.active);
    }
}
#[test]
fn successful_switch_does_not_restore_or_retry() {
    let mut provider = Provider::new();
    let mut attempt = Deadline { calls: 0 };
    let mut restore = Deadline { calls: 0 };
    assert_eq!(
        run(switch_from_access_point(
            &mut provider,
            &mut attempt,
            &mut restore
        )),
        AccessPointSwitchOutcome::StationReady
    );
    assert_eq!((attempt.calls, restore.calls), (1, 0));
    assert_eq!(
        provider.operations,
        [
            Operation::Withdraw,
            Operation::Associate,
            Operation::Address
        ]
    );
}

#[test]
fn root_can_restore_after_link_loss_under_a_fresh_finite_cleanup_budget() {
    let mut provider = Provider::new();
    let mut attempt = Deadline { calls: 0 };
    let mut cleanup = Deadline { calls: 0 };
    assert_eq!(
        run(switch_from_access_point(
            &mut provider,
            &mut attempt,
            &mut cleanup
        )),
        AccessPointSwitchOutcome::StationReady
    );
    assert_eq!(
        run(restore_access_point(&mut provider, &mut cleanup)),
        Ok(())
    );
    assert_eq!((attempt.calls, cleanup.calls), (1, 1));
    assert_eq!(
        provider.operations,
        [
            Operation::Withdraw,
            Operation::Associate,
            Operation::Address,
            Operation::Restore
        ]
    );
}
