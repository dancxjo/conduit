//! One bounded native executor around the ordinary scheduler. This module grants
//! no Body or release authority; Root must admit/release its actual task first.
use crate::{
    native_authority::NativeBindings,
    native_kernel_storage::PreparedKernel,
    native_service::{self as service, Dispatch, Invocation, Reply},
};
use alloc::boxed::Box;
use conduit_environment_runtime::{appliance::ApplianceBack, publication::PublicationSnapshot};
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_monotonic_wait::MonotonicWaitProvider;
use conduit_pulse_capture::owner::PulseReadProvider;
use core::future::Future;
use embassy_futures::{
    join::join,
    select::{select, Either},
    yield_now,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};

const PULSE_BYTES: usize = conduit_pulse_capture::PULSE_READ_MAXIMUM_BYTES as usize;
const WAIT_BYTES: usize = conduit_monotonic_wait::PARAMETER_BYTES as usize;
struct Queues {
    pulse: Channel<NoopRawMutex, Invocation<PULSE_BYTES>, 1>,
    wait: Channel<NoopRawMutex, Invocation<WAIT_BYTES>, 1>,
    replies: Channel<NoopRawMutex, Reply, 2>,
}
impl Queues {
    const fn new() -> Self {
        Self {
            pulse: Channel::new(),
            wait: Channel::new(),
            replies: Channel::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub steps: u64,
    pub submitted: [u64; 2],
    pub completed: [u64; 2],
    pub software: u64,
    pub publications: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Drained,
    Cancelled,
    Failed(k::Failure),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub exit: Exit,
    pub cleanup: Result<(), k::Failure>,
    pub progress: Progress,
}
pub use crate::native_retirement::RetiredProviders;
/// Retain these exact owners and queues through task admission and terminal
/// acknowledgement. Neither constructing nor sizing a future polls a Step.
pub struct Execution<P, W> {
    pub(crate) storage: PreparedKernel,
    pub(crate) native: NativeBindings<P, W>,
    queues: Queues,
    progress: Progress,
    begun: bool,
    retired: Option<Result<(), k::Failure>>,
}
impl<P: PulseReadProvider, W: MonotonicWaitProvider> Execution<P, W> {
    fn new(storage: PreparedKernel, native: NativeBindings<P, W>) -> Self {
        Self {
            storage,
            native,
            queues: Queues::new(),
            progress: Progress::default(),
            begun: false,
            retired: None,
        }
    }
    /// Root separately admits this exact output box and control headroom.
    /// Missing peers refuse before allocation or transfer; no owners are cloned.
    #[inline(never)]
    pub fn boxed_retained(
        storage: &mut Option<PreparedKernel>,
        native: &mut Option<NativeBindings<P, W>>,
    ) -> Result<Box<Self>, &'static str> {
        if storage.is_none() || native.is_none() {
            return Err("execution-owners-unavailable");
        }
        Ok(Box::new(Self::new(
            storage.take().expect("preflighted kernel"),
            native.take().expect("preflighted native owners"),
        )))
    }
    /// Cancel an unpolled held execution without inventing a completed run.
    pub fn cancel_held(&mut self) -> Result<Outcome, &'static str> {
        if self.begun || self.retired.is_some() {
            return Err("execution-not-unpolled");
        }
        self.begun = true;
        let cleanup = service::quiesce(&mut self.storage, &mut self.native);
        self.retired = Some(cleanup);
        Ok(Outcome {
            exit: Exit::Cancelled,
            cleanup,
            progress: self.progress,
        })
    }
    /// Retirement is post-Play custody transfer, not permission to reuse this
    /// execution or its old claims. Preflight both owners before consuming either.
    #[inline(never)]
    pub fn retire_retained(
        execution: &mut Option<Box<Self>>,
    ) -> Result<RetiredProviders<P, W>, &'static str> {
        let current = execution.as_ref().ok_or("execution-unavailable")?;
        if current.retired != Some(Ok(())) {
            return Err("execution-cleanup-unconfirmed");
        }
        crate::native_retirement::check_pair(&current.native.pulse, &current.native.wait)?;
        let Self { native, .. } = *execution.take().expect("preflighted execution");
        let mut pulse = Some(native.pulse);
        let mut wait = Some(native.wait);
        Ok(RetiredProviders::retire_pair(&mut pulse, &mut wait)
            .expect("exclusively preflighted native retirement"))
    }
    /// Size the exact unpolled invocation type, including its control/observer.
    /// The actual task pool and enclosing task frame remain separate admission.
    pub fn future_bytes(
        &mut self,
        stop: impl Future<Output = ()>,
        observer: impl for<'a> FnMut(PublicationSnapshot<'a>),
    ) -> usize {
        core::mem::size_of_val(&self.run(stop, observer))
    }
    /// Poll only after actual Root task/resource admission and authenticated
    /// release. The stop future must come from that task's scoped control owner.
    pub async fn run(
        &mut self,
        stop: impl Future<Output = ()>,
        mut observer: impl for<'a> FnMut(PublicationSnapshot<'a>),
    ) -> Outcome {
        if self.begun {
            // An abandoned invocation is not a new admission or pre-start
            // refusal. Reclaim its actual owners and keep this execution fenced.
            let (detail, cleanup) = match self.retired {
                Some(cleanup) => (26, cleanup),
                None => {
                    let cleanup = service::quiesce(&mut self.storage, &mut self.native);
                    self.retired = Some(cleanup);
                    self.queues.pulse.clear();
                    self.queues.wait.clear();
                    self.queues.replies.clear();
                    (27, cleanup)
                }
            };
            return Outcome {
                exit: Exit::Failed(failure(k::FailureCode::InvalidLifecycle, detail)),
                cleanup,
                progress: self.progress,
            };
        }
        self.begun = true;
        let exit = {
            let pump = pump(
                &mut self.storage,
                &self.queues,
                &mut self.progress,
                &mut observer,
            );
            let effects = join(
                pulses(&mut self.native.pulse, &self.queues),
                waits(&mut self.native.wait, &self.queues),
            );
            match select(stop, select(pump, effects)).await {
                Either::First(()) => Exit::Cancelled,
                Either::Second(Either::First(exit)) => exit,
                Either::Second(Either::Second(_)) => {
                    unreachable!("native effect queues do not close")
                }
            }
        }; // Drop both invocation futures before regaining/revoking the providers.
        let cleanup = if exit == Exit::Drained {
            revoke(&mut self.storage, &mut self.native)
        } else {
            service::quiesce(&mut self.storage, &mut self.native)
        };
        self.retired = Some(cleanup);
        // A terminal execution is not reusable. Retire queued inputs/results so
        // no delayed message can retain or recreate a pending invocation.
        self.queues.pulse.clear();
        self.queues.wait.clear();
        self.queues.replies.clear();
        Outcome {
            exit,
            cleanup,
            progress: self.progress,
        }
    }
}
async fn pulses<P: PulseReadProvider>(
    owner: &mut conduit_pulse_capture::owner::PulseHostCall<P>,
    queues: &Queues,
) {
    loop {
        let input = queues.pulse.receive().await;
        let reply = service::pulse(owner, input).await;
        queues.replies.send(reply).await;
    }
}
async fn waits<W: MonotonicWaitProvider>(
    owner: &mut conduit_monotonic_wait::MonotonicWaitHostCall<W>,
    queues: &Queues,
) {
    loop {
        let input = queues.wait.receive().await;
        let reply = service::wait(owner, input).await;
        queues.replies.send(reply).await;
    }
}
async fn pump(
    storage: &mut PreparedKernel,
    queues: &Queues,
    progress: &mut Progress,
    observer: &mut impl for<'a> FnMut(PublicationSnapshot<'a>),
) -> Exit {
    loop {
        let iteration = (|| {
            // At most two native completions coexist. This is transport work;
            // every completion still crosses the kernel's exact pending gate.
            for _ in 0..2 {
                let Ok(reply) = queues.replies.try_receive() else {
                    break;
                };
                let completed = service::finish(storage, reply)?;
                let index = if completed.request.node == storage.roles.pulse {
                    0
                } else {
                    1
                };
                bump(&mut progress.completed[index])?;
                if let Some(reason) = completed.failure {
                    return Err(reason);
                }
            }
            match service::next(storage)? {
                Dispatch::Pulse(input) => {
                    queues
                        .pulse
                        .try_send(input)
                        .map_err(|_| failure(k::FailureCode::StorageExhausted, 20))?;
                    bump(&mut progress.submitted[0])?;
                }
                Dispatch::Wait(input) => {
                    queues
                        .wait
                        .try_send(input)
                        .map_err(|_| failure(k::FailureCode::StorageExhausted, 21))?;
                    bump(&mut progress.submitted[1])?;
                }
                Dispatch::Software(call) => {
                    bump(&mut progress.software)?;
                    if let Some(reason) = call.failure {
                        return Err(reason);
                    }
                }
                Dispatch::None => (),
            }
            let status = storage
                .kernel
                .step()
                .map_err(|_| failure(k::FailureCode::InvalidLifecycle, 22))?;
            bump(&mut progress.steps)?;
            for driver in storage.kernel.drivers() {
                if let ApplianceBack::Publication(back) = driver {
                    if let Some(snapshot) = back.snapshot() {
                        if snapshot.sequence > progress.publications {
                            progress.publications = snapshot.sequence;
                            // Projection of committed truth only. Caller admits
                            // observer work/storage; this is no logging authority.
                            observer(snapshot);
                        }
                    }
                }
            }
            Ok(status)
        })();
        match iteration {
            Err(reason) => return Exit::Failed(reason),
            Ok(SchedulerStatus::Drained) => return Exit::Drained,
            Ok(SchedulerStatus::Cancelled) => return Exit::Cancelled,
            Ok(SchedulerStatus::Idle) if storage.kernel.pending_host_call_count() > 0 => {
                // Wake on completion without busy-spinning while a timer waits.
                // Keep this receive cancellation-safe: the reply is consumed
                // and correlated in this same branch before another await.
                let reply = queues.replies.receive().await;
                let completed = match service::finish(storage, reply) {
                    Ok(call) => call,
                    Err(reason) => return Exit::Failed(reason),
                };
                let index = if completed.request.node == storage.roles.pulse {
                    0
                } else {
                    1
                };
                if let Err(reason) = bump(&mut progress.completed[index]) {
                    return Exit::Failed(reason);
                }
                if let Some(reason) = completed.failure {
                    return Exit::Failed(reason);
                }
            }
            Ok(_) => (),
        }
        yield_now().await;
    }
}
fn revoke<P: PulseReadProvider, W: MonotonicWaitProvider>(
    storage: &mut PreparedKernel,
    native: &mut NativeBindings<P, W>,
) -> Result<(), k::Failure> {
    let pulse = native.pulse.revoke(storage.roles.pulse, k::HostCallId(0));
    let wait = native.wait.revoke(storage.roles.tick, k::HostCallId(0));
    pulse.map_err(|_| failure(k::FailureCode::HostCallDenied, 23))?;
    wait.map_err(|_| failure(k::FailureCode::HostCallDenied, 24))
}
fn bump(value: &mut u64) -> Result<(), k::Failure> {
    *value = value
        .checked_add(1)
        .ok_or(failure(k::FailureCode::IdentityCapacityExhausted, 25))?;
    Ok(())
}
const fn failure(code: k::FailureCode, detail: u16) -> k::Failure {
    k::Failure { code, detail }
}
