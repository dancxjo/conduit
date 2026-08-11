use super::InstalledScheduler;
use crate::{DeadlineReactor, DeadlineWake, TimerAdapter};
use conduit_kernel::scheduler::{HostOperationCancellation, HostOperationRequest};
use conduit_kernel::{HostOperationDisposition, HostOperationOutcome};

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
        request: HostOperationRequest,
        duration_ms: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        self.observe_now(now_ms)?;
        self.reactor
            .arm(request.into(), duration_ms, now_ms)
            .map_err(|error| format!("arm admitted deadline: {error:?}"))
    }

    pub(super) fn cancel(
        &mut self,
        cancellation: HostOperationCancellation,
        scheduler: &mut InstalledScheduler,
    ) -> Result<(), String> {
        self.reactor
            .cancel(cancellation.into())
            .map_err(|error| format!("cancel admitted deadline: {error:?}"))?;
        scheduler
            .complete_host_operation(
                cancellation.node,
                cancellation.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| format!("complete cancelled deadline: {error:?}"))
    }

    pub(super) fn complete_next<T: TimerAdapter>(
        &mut self,
        scheduler: &mut InstalledScheduler,
        timer: &mut T,
    ) -> Result<bool, String> {
        if self.reactor.is_empty() {
            return Ok(false);
        }
        let now_ms = timer
            .monotonic_now_ms()
            .ok_or_else(|| "admitted monotonic deadline Base became unavailable".to_string())?;
        self.observe_now(now_ms)?;
        let wake = match self.reactor.poll(now_ms) {
            DeadlineWake::Pending { deadline_ms } => {
                if !timer.wait_until_monotonic_ms(deadline_ms) {
                    return Err("admitted monotonic deadline wait became unavailable".to_string());
                }
                let after_wait = timer.monotonic_now_ms().ok_or_else(|| {
                    "admitted monotonic deadline clock became unavailable after wait".to_string()
                })?;
                self.observe_now(after_wait)?;
                self.reactor.poll(after_wait)
            }
            wake => wake,
        };
        let DeadlineWake::Fired(key) = wake else {
            return match wake {
                DeadlineWake::Empty => Ok(false),
                DeadlineWake::Pending { .. } => {
                    Err("monotonic deadline wait returned before its exact deadline".to_string())
                }
                DeadlineWake::Fired(_) => unreachable!(),
            };
        };
        scheduler
            .complete_host_operation(
                key.node,
                key.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| format!("complete fired deadline: {error:?}"))?;
        Ok(true)
    }

    pub(super) fn clear(&mut self) {
        self.reactor.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.reactor.is_empty()
    }

    fn observe_now(&mut self, now_ms: u64) -> Result<(), String> {
        if self.last_now_ms.is_some_and(|previous| now_ms < previous) {
            return Err("admitted monotonic deadline Base regressed or became stale".to_string());
        }
        self.last_now_ms = Some(now_ms);
        Ok(())
    }
}
