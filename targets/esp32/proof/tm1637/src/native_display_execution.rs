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
        Ok(Box::new(Self {
            storage: storage.take().expect("checked kernel"),
            owner: owner.take(),
            progress: Progress::default(),
            begun: false,
            cleanup: None,
        }))
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
        if self.begun {
            let cleanup = match self.cleanup {
                Some(c) => c,
                None => self.quiesce(),
            };
            self.cleanup = Some(cleanup);
            return Outcome {
                exit: Exit::Failed(fail(1)),
                cleanup,
                progress: self.progress,
            };
        }
        self.begun = true;
        let exit = match select(stop, self.pump()).await {
            Either::First(()) => Exit::Cancelled,
            Either::Second(exit) => exit,
        };
        let cleanup = if exit == Exit::Drained {
            self.revoke()
        } else {
            self.quiesce()
        };
        self.cleanup = Some(cleanup);
        Outcome {
            exit,
            cleanup,
            progress: self.progress,
        }
    }
    async fn pump(&mut self) -> Exit {
        loop {
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
                    Err(failure) => return Exit::Failed(failure),
                };
                let counter = if writing {
                    &mut self.progress.native_calls
                } else {
                    &mut self.progress.software_calls
                };
                let Some(next) = counter.checked_add(1) else {
                    return Exit::Failed(fail(2));
                };
                *counter = next;
                if let Some(failure) = report.failure {
                    return Exit::Failed(failure);
                }
            }
            let Some(next) = self.progress.steps.checked_add(1) else {
                return Exit::Failed(fail(3));
            };
            self.progress.steps = next;
            match self.storage.kernel.step() {
                Ok(SchedulerStatus::Drained) => return Exit::Drained,
                Ok(SchedulerStatus::Cancelled) => return Exit::Cancelled,
                Err(_) => return Exit::Failed(fail(4)),
                _ => yield_now().await,
            }
        }
    }
    pub fn retire_retained(
        execution: &mut Option<Box<Self>>,
    ) -> Result<crate::native_retirement::RetiredDisplay<P>, &'static str> {
        let current = execution.as_mut().ok_or("display-execution-unavailable")?;
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
