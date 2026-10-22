//! Finite execution mechanics for an already admitted network Host Call.
//! Root selects deadlines and whether an AP must be restored. This module
//! selects no offer, resource, credential, retry, Plan or lifecycle transition.
use core::future::Future;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationPhase {
    WithdrawAccessPoint,
    Associate,
    ConfigureAddress,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationFailure {
    Rejected,
    ProviderUnavailable,
    Deadline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StationAttemptFailure {
    pub phase: StationPhase,
    pub reason: StationFailure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeadlineExpired;

/// Root supplies one admitted finite deadline for the complete attempt, and a
/// separate finite deadline for cleanup. Expiry must drop the in-flight future.
pub trait OperationDeadline {
    fn run<T>(
        &mut self,
        work: impl Future<Output = T>,
    ) -> impl Future<Output = Result<T, DeadlineExpired>>;
}
/// Owned physical service used only after the ordinary Host Call admission.
/// Each operation must be cancellation-safe. A configured address must belong
/// to the new station attachment, never the AP's previous static address.
pub trait StationOperations {
    fn associate(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
    fn configure_address(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
}
pub trait AccessPointSwitchOperations: StationOperations {
    /// Withdraw AP services and switch to station address configuration.
    fn withdraw_access_point(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
    /// Quiesce station state, restore AP addressing, and start AP services.
    /// Success means the AP is ready; a requested restoration is insufficient.
    fn restore_access_point(&mut self) -> impl Future<Output = Result<(), StationFailure>>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessPointSwitchOutcome {
    StationReady,
    AccessPointRestored {
        failure: StationAttemptFailure,
    },
    Unavailable {
        failure: StationAttemptFailure,
        restoration: StationFailure,
    },
}

/// Association and address configuration share a single deadline. The caller
/// may publish a NetworkAttachment only after this completes successfully.
pub async fn associate_and_configure(
    provider: &mut impl StationOperations,
    deadline: &mut impl OperationDeadline,
) -> Result<(), StationAttemptFailure> {
    let mut phase = StationPhase::Associate;
    match deadline
        .run(async {
            provider.associate().await?;
            phase = StationPhase::ConfigureAddress;
            provider.configure_address().await
        })
        .await
    {
        Ok(result) => result.map_err(|reason| StationAttemptFailure { phase, reason }),
        Err(_) => Err(StationAttemptFailure {
            phase,
            reason: StationFailure::Deadline,
        }),
    }
}

/// Root invokes one AP-to-station operation. There is no retry loop. Failure
/// always attempts the separately bounded restoration before reporting outcome.
/// Link loss after success requires a new Root-owned restoration operation.
pub async fn switch_from_access_point(
    provider: &mut impl AccessPointSwitchOperations,
    attempt: &mut impl OperationDeadline,
    restoration: &mut impl OperationDeadline,
) -> AccessPointSwitchOutcome {
    let mut phase = StationPhase::WithdrawAccessPoint;
    let failure = match attempt
        .run(async {
            provider.withdraw_access_point().await?;
            phase = StationPhase::Associate;
            provider.associate().await?;
            phase = StationPhase::ConfigureAddress;
            provider.configure_address().await
        })
        .await
    {
        Ok(Ok(())) => return AccessPointSwitchOutcome::StationReady,
        Ok(Err(reason)) => StationAttemptFailure { phase, reason },
        Err(_) => StationAttemptFailure {
            phase,
            reason: StationFailure::Deadline,
        },
    };
    match restore_access_point(provider, restoration).await {
        Ok(()) => AccessPointSwitchOutcome::AccessPointRestored { failure },
        Err(restoration) => AccessPointSwitchOutcome::Unavailable {
            failure,
            restoration,
        },
    }
}

/// A separate bounded Root cleanup operation, also usable after station link
/// loss. Root must withdraw the old attachment before invoking it; this helper
/// neither preserves an obsolete attachment nor invents a new one.
pub async fn restore_access_point(
    provider: &mut impl AccessPointSwitchOperations,
    deadline: &mut impl OperationDeadline,
) -> Result<(), StationFailure> {
    match deadline.run(provider.restore_access_point()).await {
        Ok(result) => result,
        Err(_) => Err(StationFailure::Deadline),
    }
}

/// Guard each work poll with an independently supplied absolute-deadline check.
/// The surrounding clock provider must still arrange a wake at expiry. This
/// prevents effectful first polls after expiry and rejects late synchronous
/// completion; it cannot preempt synchronous native work inside one poll.
pub async fn guard_operation_deadline<T>(
    work: impl Future<Output = T>,
    mut expired: impl FnMut() -> bool,
) -> Result<T, DeadlineExpired> {
    let mut work = core::pin::pin!(work);
    core::future::poll_fn(|cx| {
        if expired() {
            return core::task::Poll::Ready(Err(DeadlineExpired));
        }
        let result = work.as_mut().poll(cx);
        if expired() {
            core::task::Poll::Ready(Err(DeadlineExpired))
        } else {
            result.map(Ok)
        }
    })
    .await
}
