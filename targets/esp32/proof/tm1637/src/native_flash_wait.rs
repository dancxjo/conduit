//! Cooperative maintenance only while the provider's original timer is pending.
//! This does not qualify a job's physical duration or establish erase authority.
use crate::{
    native_flash_jobs::{Job, WaitExecutor},
    native_flash_window::{FlashPermit, Gate, Refusal, WaitGuard},
};
use core::{future::Future, pin::pin, task::Poll};
use embassy_time::{Duration, Instant, Timer};

pub(crate) struct WaitLane<'a, J, R, E> {
    gate: &'a Gate,
    executor: WaitExecutor<'a, J, R, E>,
}
impl<'a, J: Job, R, E> WaitLane<'a, J, R, E> {
    pub(crate) fn new(gate: &'a Gate, executor: WaitExecutor<'a, J, R, E>) -> Self {
        Self { gate, executor }
    }
    pub(crate) async fn wait(
        &mut self,
        duration_ms: u64,
        mut run: impl FnMut(&mut J, &FlashPermit<'_, '_>) -> Result<R, E>,
    ) -> Result<(), Refusal> {
        let duration = Duration::from_millis(duration_ms);
        let deadline = Instant::now()
            .as_ticks()
            .checked_add(duration.as_ticks())
            .map(Instant::from_ticks)
            .ok_or(Refusal::Clock)?;
        let mut timer = pin!(Timer::at(deadline));
        let mut window: Option<WaitGuard<'_>> = None;
        let mut tried_window = false;
        let gate = self.gate;
        let executor = &mut self.executor;
        core::future::poll_fn(|context| {
            // Poll first: register the actual timer, and never admit flash once
            // it is ready. All subsequent polls use this SAME absolute deadline.
            if timer.as_mut().poll(context).is_ready() {
                drop(window.take());
                return Poll::Ready(Ok(()));
            }
            let now = Instant::now().as_millis();
            if !tried_window {
                tried_window = true;
                // Submillisecond/zero waits expose no positive safe window.
                // Arbitration refusal inhibits maintenance, not measurement.
                window = gate.begin_wait(now, deadline.as_millis()).ok();
            }
            if let Some(window) = window.as_ref() {
                // Embassy supplies a task-backed waker. Clone before entering
                // the mailbox; no user callback runs under its RefCell borrow.
                if executor.register(context.waker().clone()).is_ready() {
                    let _ = executor.execute(window, now, |job, permit| run(job, permit));
                }
            }
            Poll::Pending
        })
        .await
    }
}
