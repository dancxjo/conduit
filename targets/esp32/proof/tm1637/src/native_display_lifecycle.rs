//! Display lifecycle below authenticated Root reservation.
use super::*;
#[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
impl NativeDisplay {
    /// This acknowledges held custody only. No Step or electrical call runs.
    pub fn accept_start(&mut self) -> Result<(), &'static str> {
        if let Some(worker) = self.worker.as_ref() {
            return worker.accept_start();
        }
        let ready = self.execution.as_ref().is_some_and(|context| {
            let status = context.inspection();
            status.lifecycle == c::CapabilityLifecycle::Issued
                && status.in_flight_operations == 0
                && status.completed_operations == 0
                && context.progress() == execution::Progress::default()
        });
        crate::native_execution_gate::accept_start(&mut self.phase, false, ready)
    }
}

#[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
impl NativeDisplay {
    pub async fn cancel_held(&mut self) -> Result<(), &'static str> {
        if let Some(worker) = self.worker.as_ref() {
            worker.cancel()?;
            return embassy_time::with_timeout(embassy_time::Duration::from_secs(2), async {
                loop {
                    let status = worker.status();
                    if status.phase == "quiesced"
                        && !status.task_active
                        && !status.cleanup_failure
                        && status.native_lifecycle == "revoked"
                        && status.native_in_flight == 0
                    {
                        return Ok(());
                    }
                    if matches!(
                        status.phase,
                        "failed" | "drained" | "cleanup-refused" | "ownership-refused" | "retired"
                    ) {
                        return Err("display-task-quiescence-refused");
                    }
                    embassy_time::Timer::after_millis(5).await;
                }
            })
            .await
            .map_err(|_| "display-task-quiescence-timeout")?;
        }
        if !matches!(self.phase, "ready-held" | "started-held") {
            return Err("display-not-held");
        }
        if let Some(context) = self.execution.as_mut() {
            context.cancel_held()?.cleanup?;
            self.phase = "quiesced";
            return Ok(());
        }
        execution::Execution::cancel_prepared(
            &mut self.kernel,
            &mut self.bound,
            self.bound_write.ok_or("display-binding-unavailable")?,
        )?;
        self.phase = "quiesced";
        Ok(())
    }
    pub fn retire_held(
        &mut self,
        issuer: &mut crate::native_authority::RootIssuer,
        active: &c::ActivePlayIdentity,
    ) -> Result<(), &'static str> {
        if let Some(worker) = self.worker.as_ref() {
            if self.phase == "retired" {
                return Err("display-already-retired");
            }
            if self.retired.is_none() {
                self.retired = Some(worker.retire()?);
            }
            issuer
                .finish_display_play(
                    active,
                    self.retired.as_ref().expect("returned task custody"),
                )
                .map_err(|_| "display-retirement-scope")?;
            self.phase = "retired";
            return Ok(());
        }
        if self.phase != "quiesced" {
            return Err("display-not-quiesced");
        }
        if self.retired.is_none() {
            self.retired = Some(if self.execution.is_some() {
                execution::Execution::retire_retained(&mut self.execution)?
            } else {
                crate::native_retirement::RetiredDisplay::retire_boxed(&mut self.bound)?
            });
        }
        issuer
            .finish_display_play(
                active,
                self.retired.as_ref().expect("returned display custody"),
            )
            .map_err(|_| "display-retirement-scope")?;
        self.kernel = None;
        self.bound_write = None;
        self.phase = "retired";
        Ok(())
    }
}

#[cfg(any(feature = "display-ready-root", feature = "display-physical-check"))]
impl NativeDisplay {
    pub fn release(&self) -> Result<(), &'static str> {
        self.worker
            .as_ref()
            .ok_or("display-task-unavailable")?
            .release()
    }
}
