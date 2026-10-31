//! Bounded target execution around the ordinary output kernel. Root owns task
//! admission, authenticated Start/Release and typed Fore admission separately.
use super::kernel::Prepared;
use super::service::DisplayService;
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_tm1637_wire::{owner::WriteHostCall, OpenDrain};
use core::future::Future;
use embassy_futures::{
    select::{select, Either},
    yield_now,
};
#[path = "native_display_fore.rs"]
// Not activated by Root until its actual local caller/consumer are admitted.
#[allow(dead_code)]
pub mod fore;
#[cfg(feature = "display-physical-check")]
#[path = "native_display_session_fore.rs"]
mod session_fore;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub steps: u64,
    pub native_calls: u64,
    pub software_calls: u64,
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
    pub cleanup: Result<(), &'static str>,
    pub progress: Progress,
}
pub struct Execution<P: OpenDrain> {
    storage: Prepared,
    owner: Option<Box<WriteHostCall<P>>>,
    progress: Progress,
    begun: bool,
    cleanup: Option<Result<(), &'static str>>,
    #[cfg(feature = "display-physical-check")]
    invocation: u64,
}
impl<P: OpenDrain> Execution<P> {
    /// Root admits this box and preparation headroom before transfer. Missing
    /// or foreign owners refuse without consuming either side; no Step runs.
    pub fn boxed_retained(
        storage: &mut Option<Prepared>,
        owner: &mut Option<Box<WriteHostCall<P>>>,
    ) -> Result<Box<Self>, &'static str> {
        let identity = storage
            .as_ref()
            .ok_or("display-kernel-unavailable")?
            .identity;
        let scope = &owner
            .as_ref()
            .ok_or("display-owner-unavailable")?
            .inspection()
            .scope;
        if identity.host != c::AssignedIdentity::from_text(scope.host_id.as_str())
            || identity.boot != c::AssignedIdentity::from_text(scope.boot_id.as_str())
            || identity.plan != c::AssignedIdentity::from_text(scope.plan_id.as_str())
        {
            return Err("display-execution-scope");
        }
        #[cfg(feature = "display-physical-check")]
        super::evidence_task::prepare()?;
        Ok(Box::new(Self {
            storage: storage.take().expect("checked kernel"),
            owner: owner.take(),
            progress: Progress::default(),
            begun: false,
            cleanup: None,
            #[cfg(feature = "display-physical-check")]
            invocation: 0,
        }))
    }
    /// Cancel actual dormant custody even if later kernel preparation refused.
    /// A wrong ordinal/scope preserves both sides; no Step or GPIO call runs.
    pub fn cancel_prepared(
        storage: &mut Option<Prepared>,
        owner: &mut Option<Box<WriteHostCall<P>>>,
        write: k::NodeId,
    ) -> Result<(), &'static str> {
        let bound = owner.as_mut().ok_or("display-binding-unavailable")?;
        if let Some(storage) = storage.as_ref() {
            let scope = &bound.inspection().scope;
            let id = c::AssignedIdentity::from_text;
            if storage.write != write
                || storage.identity.host != id(scope.host_id.as_str())
                || storage.identity.boot != id(scope.boot_id.as_str())
                || storage.identity.plan != id(scope.plan_id.as_str())
            {
                return Err("display-execution-scope");
            }
        }
        bound
            .revoke(write, k::HostCallId(0))
            .map_err(|_| "display-revocation")?;
        if let Some(storage) = storage.as_mut() {
            storage
                .kernel
                .cancel()
                .map_err(|_| "display-cancellation")?;
        }
        Ok(())
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.owner
            .as_ref()
            .expect("retained display owner")
            .inspection()
    }
    pub fn progress(&self) -> Progress {
        self.progress
    }
    fn revoke(&mut self) -> Result<(), &'static str> {
        self.owner
            .as_mut()
            .ok_or("display-owner-unavailable")?
            .revoke(self.storage.write, k::HostCallId(0))
            .map_err(|_| "display-revocation")
    }
    fn quiesce(&mut self) -> Result<(), &'static str> {
        let revoke = self.revoke();
        let cancel = self
            .storage
            .kernel
            .cancel()
            .map_err(|_| "display-cancellation");
        revoke?;
        cancel
    }
    pub fn cancel_held(&mut self) -> Result<Outcome, &'static str> {
        if self.begun {
            return Err("display-already-begun");
        }
        self.begun = true;
        let cleanup = self.quiesce();
        self.cleanup = Some(cleanup);
        Ok(Outcome {
            exit: Exit::Cancelled,
            cleanup,
            progress: self.progress,
        })
    }
    pub async fn run(&mut self, stop: impl Future<Output = ()>) -> Outcome {
        self.run_with_idle(stop, yield_now).await
    }
    /// The task supplies an admitted input wakeup wait. An unbound Fore may
    /// remain pending until cancellation; it must not spin an idle MCU kernel.
    pub async fn run_with_idle<F: Future<Output = ()>>(
        &mut self,
        stop: impl Future<Output = ()>,
        mut idle: impl FnMut() -> F,
    ) -> Outcome {
        if let Some(outcome) = self.begin_run() {
            return outcome;
        }
        let exit = match select(stop, self.pump(&mut idle)).await {
            Either::First(()) => Exit::Cancelled,
            Either::Second(exit) => exit,
        };
        self.finish_run(exit)
    }
    fn begin_run(&mut self) -> Option<Outcome> {
        if self.begun {
            let cleanup = match self.cleanup {
                Some(c) => c,
                None => self.quiesce(),
            };
            self.cleanup = Some(cleanup);
            return Some(Outcome {
                exit: Exit::Failed(fail(1)),
                cleanup,
                progress: self.progress,
            });
        }
        self.begun = true;
        None
    }
    fn finish_run(&mut self, exit: Exit) -> Outcome {
        let mut cleanup = if exit == Exit::Drained {
            self.revoke()
        } else {
            self.quiesce()
        };
        #[cfg(feature = "display-physical-check")]
        if self.stage_evidence().is_err() {
            cleanup = Err("display-evidence-retention");
        }
        self.cleanup = Some(cleanup);
        Outcome {
            exit,
            cleanup,
            progress: self.progress,
        }
    }
    async fn pump<F: Future<Output = ()>>(&mut self, idle: &mut impl FnMut() -> F) -> Exit {
        loop {
            match self.advance() {
                Ok(SchedulerStatus::Drained) => return Exit::Drained,
                Ok(SchedulerStatus::Cancelled) => return Exit::Cancelled,
                Ok(SchedulerStatus::Idle) => idle().await,
                Err(exit) => return exit,
                _ => yield_now().await,
            }
        }
    }
    /// One ordinary scheduler Step and at most its existing pending Host Call.
    /// Fore adapters supply values; they do not implement another scheduler.
    fn advance(&mut self) -> Result<SchedulerStatus, Exit> {
        #[cfg(feature = "display-physical-check")]
        super::task::mark_running();
        let mut service = DisplayService {
            kernel: &mut self.storage.kernel,
            selectors: &mut self.storage.selectors,
            write: self.storage.write,
            owner: self.owner.as_mut().expect("retained owner"),
        };
        if let Some(pending) = service.take() {
            let writing = pending.request().node == self.storage.write;
            let report = match service.service(pending, || false) {
                Ok(report) => report,
                Err(failure) => return Err(Exit::Failed(failure)),
            };
            let counter = if writing {
                &mut self.progress.native_calls
            } else {
                &mut self.progress.software_calls
            };
            let Some(next) = counter.checked_add(1) else {
                return Err(Exit::Failed(fail(2)));
            };
            *counter = next;
            if let Some(failure) = report.failure {
                return Err(Exit::Failed(failure));
            }
        }
        let Some(next) = self.progress.steps.checked_add(1) else {
            return Err(Exit::Failed(fail(3)));
        };
        self.progress.steps = next;
        self.storage
            .kernel
            .step()
            .map_err(|_| Exit::Failed(fail(4)))
    }
    pub fn retire_retained(
        execution: &mut Option<Box<Self>>,
    ) -> Result<crate::native_retirement::RetiredDisplay<P>, &'static str> {
        let current = execution.as_mut().ok_or("display-execution-unavailable")?;
        #[cfg(feature = "display-physical-check")]
        {
            current.stage_evidence()?;
            if super::evidence_task::pending() {
                return Err("display-evidence-outstanding");
            }
        }
        if current.cleanup != Some(Ok(())) {
            return Err("display-cleanup-unconfirmed");
        }
        let retired = crate::native_retirement::RetiredDisplay::retire_boxed(&mut current.owner)?;
        *execution = None;
        Ok(retired)
    }
}
const fn fail(detail: u16) -> k::Failure {
    k::Failure {
        code: k::FailureCode::InvalidLifecycle,
        detail,
    }
}
