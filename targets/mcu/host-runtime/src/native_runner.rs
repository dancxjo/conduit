//! Concurrent service of one already Started fragment's original kernel/Calls.
//! This runner neither prepares nor admits work. Independent fragments retain
//! independent runners, so output failure need not cancel local measurement.
#[path = "native_activity.rs"]
mod activity;
pub use activity::NativeActivity;
#[path = "native_sync_join.rs"]
mod sync_join;
#[path = "native_sync.rs"]
pub mod synchronous;

use crate::{
    execution::StoredExecutionKernel,
    execution_factory::Calls,
    native_dispatch::{self, Dispatch},
};
use conduit_kernel::scheduler::{SchedulerError, SchedulerStatus, StepBack};
use conduit_kernel::{HostCallId, SignSink};
use conduit_monotonic_wait::{MonotonicWaitHostCall, MonotonicWaitProvider};
use conduit_pulse_capture::owner::{PulseHostCall, PulseReadProvider};
use conduit_tm1637_wire::{owner::WriteHostCall, OpenDrain};
use core::{
    cell::{Cell, RefCell},
    future::{poll_fn, Future},
    pin::{pin, Pin},
    task::Poll,
};

pub trait CooperativeYield {
    fn pause(&self) -> impl Future<Output = ()>;
}
/// Root's local Fore/Sign bridge. Completion is the exact scheduler completion,
/// never carrier delivery. Hooks are synchronous under the exclusive kernel loan.
pub trait Observer<K> {
    type Error;
    fn before_step(&mut self, kernel: &mut K) -> Result<(), Self::Error>;
    fn completed(&mut self, kernel: &mut K, dispatch: Dispatch) -> Result<(), Self::Error>;
}
#[derive(Debug)]
pub enum Refusal<E> {
    Kernel(SchedulerError),
    Observer(E),
    Synchronous(synchronous::Refusal),
    Retirement {
        kernel: Option<SchedulerError>,
        native_revocation_failed: bool,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Drained,
}

/// Construct only from `StartedLocalWorkset::kernels_mut()` and its bound native
/// Calls. The caller retains original execution authority and cancellation policy.
pub struct Runner<
    'a,
    D: StepBack<P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    Y,
    O,
    F,
    L: SignSink,
> {
    kernel: RefCell<&'a mut StoredExecutionKernel<D, L, N, C, P, R, T, H>>,
    observer: RefCell<&'a mut O>,
    yield_to_host: &'a Y,
    cancelled: &'a F,
    done: Cell<bool>,
    activity: Option<&'a NativeActivity>,
}
impl<
        'a,
        D: StepBack<P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        Y: CooperativeYield,
        O: Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H>>,
        F: Fn() -> bool,
        L: SignSink,
    > Runner<'a, D, N, C, P, R, T, H, Y, O, F, L>
{
    pub fn new(
        kernel: &'a mut StoredExecutionKernel<D, L, N, C, P, R, T, H>,
        observer: &'a mut O,
        yield_to_host: &'a Y,
        cancelled: &'a F,
    ) -> Self {
        Self {
            kernel: RefCell::new(kernel),
            observer: RefCell::new(observer),
            yield_to_host,
            cancelled,
            done: Cell::new(false),
            activity: None,
        }
    }
    /// Share one single-core timing exclusion across every GPIO runner and the
    /// blocking provider actor. This does not confer resource or Start authority.
    pub fn with_activity(mut self, activity: &'a NativeActivity) -> Self {
        self.activity = Some(activity);
        self
    }
    async fn scheduler(&self) -> Result<(), Refusal<O::Error>> {
        loop {
            if (self.cancelled)() {
                return Err(Refusal::Kernel(SchedulerError::Cancelled));
            }
            if let Ok(mut kernel) = self.kernel.try_borrow_mut() {
                self.observer
                    .borrow_mut()
                    .before_step(&mut **kernel)
                    .map_err(Refusal::Observer)?;
                match kernel.step().map_err(Refusal::Kernel)? {
                    SchedulerStatus::Drained => {
                        self.done.set(true);
                        return Ok(());
                    }
                    SchedulerStatus::Cancelled => {
                        return Err(Refusal::Kernel(SchedulerError::Cancelled))
                    }
                    _ => {}
                }
            }
            self.yield_to_host.pause().await;
        }
    }
    // Intentional: result/Sign preflight requires this exclusive loan through
    // the bounded cancellable effect. Every peer uses try_borrow_mut; none may
    // invalidate custody or panic on contention. Provider futures quiesce first.
    #[allow(clippy::await_holding_refcell_ref)]
    async fn pulse<A: PulseReadProvider>(
        &self,
        call: Option<&mut PulseHostCall<A>>,
    ) -> Result<(), Refusal<O::Error>> {
        let mut call = call;
        while !self.done.get() {
            let timing = self.activity.and_then(NativeActivity::try_timing);
            if self.activity.is_some() && timing.is_none() {
                self.yield_to_host.pause().await;
                continue;
            }
            if let (Some(call), Ok(mut kernel)) =
                (call.as_deref_mut(), self.kernel.try_borrow_mut())
            {
                // This exact exclusive loan survives the provider await. Other
                // actors skip this kernel until result/Sign publication finishes.
                let dispatch = native_dispatch::pulse(&mut **kernel, call)
                    .await
                    .map_err(Refusal::Kernel)?;
                if matches!(dispatch, Dispatch::Completed { .. }) {
                    self.observer
                        .borrow_mut()
                        .completed(&mut **kernel, dispatch)
                        .map_err(Refusal::Observer)?;
                }
            }
            drop(timing);
            self.yield_to_host.pause().await;
        }
        Ok(())
    }
    async fn write<A: OpenDrain>(
        &self,
        call: Option<&mut WriteHostCall<A>>,
    ) -> Result<(), Refusal<O::Error>> {
        let mut call = call;
        while !self.done.get() {
            let timing = self.activity.and_then(NativeActivity::try_timing);
            if self.activity.is_some() && timing.is_none() {
                self.yield_to_host.pause().await;
                continue;
            }
            if let (Some(call), Ok(mut kernel)) =
                (call.as_deref_mut(), self.kernel.try_borrow_mut())
            {
                let dispatch = native_dispatch::write(&mut **kernel, call, self.cancelled)
                    .map_err(Refusal::Kernel)?;
                if matches!(dispatch, Dispatch::Completed { .. }) {
                    self.observer
                        .borrow_mut()
                        .completed(&mut **kernel, dispatch)
                        .map_err(Refusal::Observer)?;
                }
            }
            drop(timing);
            self.yield_to_host.pause().await;
        }
        Ok(())
    }
    async fn wait<A: MonotonicWaitProvider>(
        &self,
        call: &mut MonotonicWaitHostCall<A>,
    ) -> Result<(), Refusal<O::Error>> {
        while !self.done.get() {
            let pending = {
                match self.kernel.try_borrow_mut() {
                    Ok(mut kernel) => {
                        native_dispatch::begin_wait(&mut **kernel, call).map_err(Refusal::Kernel)?
                    }
                    Err(_) => None,
                }
            };
            if let Some(pending) = pending {
                // The owned eight-byte parameter/request survives while this
                // kernel remains available to its scheduler and other natives.
                let outcome = pending.invoke(call).await;
                loop {
                    if let Ok(mut kernel) = self.kernel.try_borrow_mut() {
                        let dispatch =
                            native_dispatch::complete_wait(&mut **kernel, &pending, outcome)
                                .map_err(Refusal::Kernel)?;
                        if matches!(dispatch, Dispatch::Completed { .. }) {
                            self.observer
                                .borrow_mut()
                                .completed(&mut **kernel, dispatch)
                                .map_err(Refusal::Observer)?;
                        }
                        break;
                    }
                    self.yield_to_host.pause().await;
                }
            }
            self.yield_to_host.pause().await;
        }
        Ok(())
    }
    async fn waits<A: MonotonicWaitProvider, const W: usize>(
        &self,
        calls: [&mut MonotonicWaitHostCall<A>; W],
    ) -> Result<(), Refusal<O::Error>> {
        let futures = calls.map(|call| self.wait(call));
        let mut futures = pin!(futures);
        let mut done = [false; W];
        poll_fn(|cx| {
            // SAFETY: the array is pinned for this entire await. Elements are
            // never moved/replaced; all are dropped in place before return.
            let entries = unsafe { futures.as_mut().get_unchecked_mut() };
            for (i, future) in entries.iter_mut().enumerate() {
                if !done[i] {
                    if let Poll::Ready(result) = unsafe { Pin::new_unchecked(future) }.poll(cx) {
                        done[i] = true;
                        if let Err(error) = result {
                            return Poll::Ready(Err(error));
                        }
                    }
                }
            }
            if done.iter().all(|done| *done) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
    async fn synchronous<S: synchronous::SynchronousHostCall>(
        &self,
        service: Option<&mut synchronous::Service<'_, S>>,
    ) -> Result<(), Refusal<O::Error>> {
        let Some(service) = service else {
            return Ok(());
        };
        let activity = self
            .activity
            .ok_or(Refusal::Synchronous(synchronous::Refusal::MissingActivity))?;
        while !self.done.get() {
            if let Ok(mut kernel) = self.kernel.try_borrow_mut() {
                let dispatch = service
                    .dispatch(&mut **kernel, activity)
                    .map_err(Refusal::Synchronous)?;
                if matches!(dispatch, Dispatch::Completed { .. }) {
                    self.observer
                        .borrow_mut()
                        .completed(&mut **kernel, dispatch)
                        .map_err(Refusal::Observer)?;
                }
            }
            self.yield_to_host.pause().await;
        }
        Ok(())
    }
    /// Return only after all provider futures have quiesced. On refusal revoke
    /// this fragment's owners, then cancel this kernel; other fragments continue.
    pub async fn run<
        A: PulseReadProvider,
        B: OpenDrain,
        W: MonotonicWaitProvider,
        const Q: usize,
    >(
        self,
        calls: &mut Calls<A, B, W, Q>,
    ) -> Result<Exit, Refusal<O::Error>> {
        self.run_services(
            calls,
            None::<&mut synchronous::Service<'_, synchronous::NoService>>,
        )
        .await
    }
    /// Add one actual independently bound synchronous native owner. Its result
    /// buffer and provider are funded before Start; no factory/admission occurs.
    /// `with_activity` must name the same interlock used by every GPIO runner.
    pub async fn run_with_sync_calls<
        A: PulseReadProvider,
        B: OpenDrain,
        W: MonotonicWaitProvider,
        const Q: usize,
        S: synchronous::SynchronousHostCall,
    >(
        self,
        calls: &mut Calls<A, B, W, Q>,
        service: &mut synchronous::Service<'_, S>,
    ) -> Result<Exit, Refusal<O::Error>> {
        self.run_services(calls, Some(service)).await
    }
    async fn run_services<
        A: PulseReadProvider,
        B: OpenDrain,
        W: MonotonicWaitProvider,
        const Q: usize,
        S: synchronous::SynchronousHostCall,
    >(
        self,
        calls: &mut Calls<A, B, W, Q>,
        mut service: Option<&mut synchronous::Service<'_, S>>,
    ) -> Result<Exit, Refusal<O::Error>> {
        let validation = service.as_ref().map_or(Ok(()), |service| {
            if self.activity.is_none() {
                return Err(synchronous::Refusal::MissingActivity);
            }
            let node = service.node();
            if calls.pulse.as_ref().is_some_and(|c| c.node() == node)
                || calls.write.as_ref().is_some_and(|c| c.node() == node)
                || calls.waits.iter().any(|c| c.node() == node)
            {
                return Err(synchronous::Refusal::ConflictingNode);
            }
            Ok(())
        });
        let result = match validation {
            Err(error) => Err(Refusal::Synchronous(error)),
            Ok(()) => {
                crate::native_supervisor::supervise(
                    self.scheduler(),
                    self.pulse(calls.pulse.as_mut()),
                    self.write(calls.write.as_mut()),
                    sync_join::join(
                        self.waits(calls.waits.each_mut()),
                        self.synchronous(service.as_deref_mut()),
                    ),
                )
                .await
            }
        };
        // supervise owns and drops every child future before returning here.
        let mut revoked = true;
        if let Some(service) = service {
            revoked &= service.revoke().is_ok();
        }
        if let Some(call) = calls.pulse.as_mut() {
            revoked &= call.revoke(call.node(), HostCallId(0)).is_ok();
        }
        if let Some(call) = calls.write.as_mut() {
            revoked &= call.revoke(call.node(), HostCallId(0)).is_ok();
        }
        for call in &mut calls.waits {
            revoked &= call.revoke(call.node(), HostCallId(0)).is_ok();
        }
        let kernel = self.kernel.into_inner();
        let cancellation = if result.is_err() {
            kernel.cancel().err()
        } else {
            None
        };
        if !revoked || cancellation.is_some() {
            return Err(Refusal::Retirement {
                kernel: cancellation,
                native_revocation_failed: !revoked,
            });
        }
        result.map(|()| Exit::Drained)
    }
}
