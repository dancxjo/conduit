use super::InstalledScheduler;
use crate::{DeadlineReactor, DeadlineWake, TimerAdapter};
use conduit_kernel::scheduler::{HostCallCancellation, HostCallRequest};
use conduit_kernel::{HostCallDisposition, HostCallOutcome};

#[derive(Debug)]
pub(super) enum InstalledDeadlineError {
    Message(&'static str),
    Reactor(&'static str, crate::DeadlineReactorError),
    Scheduler(&'static str, conduit_kernel::scheduler::SchedulerError),
}
impl core::fmt::Display for InstalledDeadlineError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Reactor(message, error) => write!(formatter, "{message}: {error:?}"),
            Self::Scheduler(message, error) => write!(formatter, "{message}: {error:?}"),
        }
    }
}
impl From<InstalledDeadlineError> for String {
    fn from(error: InstalledDeadlineError) -> Self {
        error.to_string()
    }
}

pub(super) struct InstalledDeadlineHost<const SLOTS: usize> {
    reactor: DeadlineReactor<SLOTS>,
    last_now_ms: Option<u64>,
}

impl<const SLOTS: usize> InstalledDeadlineHost<SLOTS> {
    pub(super) const fn new() -> Self {
        Self {
            reactor: DeadlineReactor::new(),
            last_now_ms: None,
        }
    }

    pub(super) fn arm(
        &mut self,
        request: HostCallRequest,
        duration_ms: u64,
        now_ms: u64,
    ) -> Result<(), InstalledDeadlineError> {
        self.observe_now(now_ms)?;
        self.reactor
            .arm(request.into(), duration_ms, now_ms)
            .map_err(|error| InstalledDeadlineError::Reactor("arm admitted deadline", error))
    }

    pub(super) fn cancel(
        &mut self,
        cancellation: HostCallCancellation,
        scheduler: &mut InstalledScheduler,
    ) -> Result<(), InstalledDeadlineError> {
        self.reactor
            .cancel(cancellation.into())
            .map_err(|error| InstalledDeadlineError::Reactor("cancel admitted deadline", error))?;
        scheduler
            .complete_host_call(
                cancellation.node,
                cancellation.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| {
                InstalledDeadlineError::Scheduler("complete cancelled deadline", error)
            })
    }

    pub(super) fn complete_next<T: TimerAdapter>(
        &mut self,
        scheduler: &mut InstalledScheduler,
        timer: &mut T,
    ) -> Result<bool, InstalledDeadlineError> {
        if self.reactor.is_empty() {
            return Ok(false);
        }
        let now_ms = timer.monotonic_now_ms().ok_or({
            InstalledDeadlineError::Message("admitted monotonic deadline Base became unavailable")
        })?;
        self.observe_now(now_ms)?;
        let wake = match self.reactor.poll(now_ms) {
            DeadlineWake::Pending { deadline_ms } => {
                if !timer.wait_until_monotonic_ms(deadline_ms) {
                    return Err(InstalledDeadlineError::Message(
                        "admitted monotonic deadline wait became unavailable",
                    ));
                }
                let after_wait = timer.monotonic_now_ms().ok_or({
                    InstalledDeadlineError::Message(
                        "admitted monotonic deadline clock became unavailable after wait",
                    )
                })?;
                self.observe_now(after_wait)?;
                self.reactor.poll(after_wait)
            }
            wake => wake,
        };
        Self::complete_wake(scheduler, wake)
    }

    /// Pending model I/O must not wait for an unrelated future deadline.
    pub(super) fn complete_ready<T: TimerAdapter>(
        &mut self,
        scheduler: &mut InstalledScheduler,
        timer: &mut T,
    ) -> Result<bool, InstalledDeadlineError> {
        if self.reactor.is_empty() {
            return Ok(false);
        }
        let now_ms = timer.monotonic_now_ms().ok_or({
            InstalledDeadlineError::Message("admitted monotonic deadline Base became unavailable")
        })?;
        self.observe_now(now_ms)?;
        Self::complete_wake(scheduler, self.reactor.poll(now_ms))
    }

    fn complete_wake(
        scheduler: &mut InstalledScheduler,
        wake: DeadlineWake,
    ) -> Result<bool, InstalledDeadlineError> {
        let DeadlineWake::Fired(key) = wake else {
            return match wake {
                DeadlineWake::Empty => Ok(false),
                DeadlineWake::Pending { .. } => Ok(false),
                DeadlineWake::Fired(_) => unreachable!(),
            };
        };
        scheduler
            .complete_host_call(
                key.node,
                key.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| InstalledDeadlineError::Scheduler("complete fired deadline", error))?;
        Ok(true)
    }

    pub(super) fn clear(&mut self) {
        self.reactor.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.reactor.is_empty()
    }

    fn observe_now(&mut self, now_ms: u64) -> Result<(), InstalledDeadlineError> {
        if self.last_now_ms.is_some_and(|previous| now_ms < previous) {
            return Err(InstalledDeadlineError::Message(
                "admitted monotonic deadline Base regressed or became stale",
            ));
        }
        self.last_now_ms = Some(now_ms);
        Ok(())
    }
}
