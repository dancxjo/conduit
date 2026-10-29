//! Dormant execution backing and actual worker registration.
use super::*;
impl NativeDisplay {
    pub fn context_bytes(&self) -> Option<usize> {
        #[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
        {
            self.execution
                .as_ref()
                .map(|_| core::mem::size_of::<execution::Execution<DisplayPins<'static>>>())
                .or_else(|| {
                    self.worker
                        .as_ref()
                        .filter(|w| w.status().native_custody)
                        .map(|w| w.status().context_bytes)
                })
        }
        #[cfg(not(any(feature = "display-ready-root", feature = "display-physical-check")))]
        {
            None
        }
    }
}
#[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
impl NativeDisplay {
    pub fn prepare_context(&mut self, available: usize) -> Result<(), &'static str> {
        if self.execution.is_some() {
            return Err("display-context-already-prepared");
        }
        if available
            < core::mem::size_of::<execution::Execution<DisplayPins<'static>>>()
                + 8192
                + if cfg!(feature = "display-physical-check") {
                    2064
                } else {
                    0
                }
        {
            return Err("display-context-capacity");
        }
        self.execution = Some(execution::Execution::boxed_retained(
            &mut self.kernel,
            &mut self.bound,
        )?);
        self.bound_write = None;
        Ok(())
    }
}

impl NativeDisplay {
    pub fn task_registered(&self) -> bool {
        #[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
        {
            self.worker.is_some()
        }
        #[cfg(not(any(feature = "display-ready-root", feature = "display-physical-check")))]
        {
            false
        }
    }
}
#[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
impl NativeDisplay {
    pub fn task_status(&self) -> Option<DisplayTaskStatus> {
        self.worker.as_ref().map(|w| w.status())
    }
    pub async fn register_task(
        &mut self,
        spawner: embassy_executor::Spawner,
    ) -> Result<(), &'static str> {
        if self.worker.is_some() {
            return Err("display-task-already-registered");
        }
        self.worker = Some(task::prepare(
            &mut self.execution,
            spawner,
            #[cfg(feature = "display-physical-check")]
            None,
        )?);
        let worker = self.worker.as_ref().expect("registered display task");
        if embassy_time::with_timeout(embassy_time::Duration::from_secs(2), worker.wait_ready())
            .await
            .is_err()
        {
            let _ = worker.cancel();
            return Err("display-task-ready-timeout");
        }
        if worker.status().phase != "ready-held" {
            return Err("display-task-not-ready");
        }
        self.phase = "task-owned";
        Ok(())
    }
}

#[cfg(feature = "display-physical-check")]
impl NativeDisplay {
    pub fn copy_evidence(
        &self,
        output: &mut [u8],
    ) -> Result<Option<(crate::native_owner_control::EvidenceReference, usize)>, &'static str> {
        if let Some(worker) = self.worker.as_ref() {
            worker.stage_terminal_evidence()?;
        } else if let Some(execution) = self.execution.as_ref() {
            execution.stage_evidence()?;
        }
        evidence_task::copy(output)
    }
    pub fn accept_evidence(
        &self,
        page: crate::native_owner_control::EvidenceReference,
    ) -> Result<(), &'static str> {
        evidence_task::accept(page)
    }
}
