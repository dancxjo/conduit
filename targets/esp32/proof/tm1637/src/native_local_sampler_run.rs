//! Transfer the actual dormant local sampler into the ordinary held worker.
//! Shared Body start, durable checkpoint and coordinated release remain Root's
//! responsibility. Registration never polls the sampler's first Step.
use super::{DeviceExecution, NativeRoot};
use conduit_core as c;
#[path = "native_local_face_scope.rs"]
mod scope;

type Bindings = crate::native_authority::NativeBindings<
    conduit_esp32_pulse_capture::Esp32PulseCapture<'static>,
    crate::native_wait::EspWait,
>;

impl NativeRoot {
    /// Prepare exactly one worker, preserving incoming owners on preflight
    /// refusal. After transfer, Root retains execution or the actual task handle
    /// even if task registration/readiness refuses. Caller must not release it
    /// before both local plots have admitted the same Body Play and persisted
    /// the resulting biography when durable custody is enabled.
    pub(crate) async fn prepare_local_sampler_worker(
        &mut self,
        kernel: &mut Option<crate::native_kernel::PreparedKernel>,
        native: &mut Option<Bindings>,
        publisher: crate::native_face::Publisher,
        active: &c::ActivePlayIdentity,
        fragment: &c::FragmentId,
    ) -> Result<(), &'static str> {
        let mut publisher = Some(publisher);
        self.prepare_local_sampler_worker_inner(
            kernel,
            native,
            &mut publisher,
            active,
            fragment,
            None,
        )
        .await
    }
    pub(crate) async fn prepare_returned_sampler_worker(
        &mut self,
        kernel: &mut Option<crate::native_kernel::PreparedKernel>,
        native: &mut Option<Bindings>,
        publisher: &mut Option<crate::native_face::Publisher>,
        active: &c::ActivePlayIdentity,
        fragment: &c::FragmentId,
        previous: &mut Option<crate::native_task::Handle>,
    ) -> Result<(), &'static str> {
        self.prepare_local_sampler_worker_inner(
            kernel,
            native,
            publisher,
            active,
            fragment,
            Some(previous),
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn prepare_local_sampler_worker_inner(
        &mut self,
        kernel: &mut Option<crate::native_kernel::PreparedKernel>,
        native: &mut Option<Bindings>,
        publisher: &mut Option<crate::native_face::Publisher>,
        active: &c::ActivePlayIdentity,
        fragment: &c::FragmentId,
        previous: Option<&mut Option<crate::native_task::Handle>>,
    ) -> Result<(), &'static str> {
        if self.worker.is_some()
            || self.execution.is_some()
            || self.native_bindings.is_some()
            || self.appliance_kernel.is_some()
            || self.owner_reservation.is_some()
            || self.retired_native.is_some()
        {
            return Err("local-sampler-already-prepared");
        }
        let body = self.body.as_ref().ok_or("local-sampler-body")?;
        if body
            .realization()
            .is_none_or(|current| current.play.is_some())
        {
            return Err("local-sampler-body-already-started");
        }
        if active.host_id != self.host || active.boot_id != self.boot {
            return Err("local-sampler-current-boot");
        }
        scope::check_installed(
            body,
            &crate::native_software::source_identity(),
            fragment,
            active,
            c::OfferGeneration(self.offer_generation),
        )?;
        if self.face_reader.is_none() {
            return Err("local-sampler-face-reader");
        }
        let (_, cadence) = crate::native_software::source_facts();
        if cadence.count != 960 || cadence.period_ms != 60_000 {
            return Err("local-sampler-cadence");
        }
        let bindings = native.as_ref().ok_or("local-sampler-native-owners")?;
        for inspection in [bindings.pulse.inspection(), bindings.wait.inspection()] {
            let scope = &inspection.scope;
            if inspection.lifecycle != c::CapabilityLifecycle::Issued
                || inspection.completed_operations != 0
                || inspection.in_flight_operations != 0
                || scope.host_id != active.host_id
                || scope.boot_id != active.boot_id
                || scope.plan_id != active.plan_id
                || scope.active_play_id != active.active_play_id
                || u64::from(scope.maximum_operations) != cadence.count
            {
                return Err("local-sampler-native-scope");
            }
        }
        if kernel.is_none() {
            return Err("local-sampler-kernel");
        }
        let spawner = self.worker_spawner.ok_or("worker-executor-unavailable")?;
        super::memory_check::mark(
            "before-local-sampler-execution-context",
            core::mem::size_of::<DeviceExecution>(),
        );
        let memory = esp_alloc::HEAP.stats();
        if memory.size.saturating_sub(memory.current_usage)
            < core::mem::size_of::<DeviceExecution>() + 8192
        {
            return Err("execution-context-capacity");
        }
        self.execution = Some(DeviceExecution::boxed_retained(kernel, native)?);
        self.worker = Some(match previous {
            Some(previous) => {
                let handle = previous.take().ok_or("renewal-retired-sampler-handle")?;
                match handle.prepare_next(&mut self.execution, spawner, publisher) {
                    Ok(handle) => handle,
                    Err((handle, reason)) => {
                        *previous = Some(handle);
                        return Err(reason);
                    }
                }
            }
            None => crate::native_task::prepare(
                &mut self.execution,
                spawner,
                publisher.take().ok_or("local-sampler-publisher")?,
            )?,
        });
        let worker = self
            .worker
            .as_ref()
            .expect("registered actual sampler task");
        if embassy_time::with_timeout(embassy_time::Duration::from_secs(2), worker.wait_ready())
            .await
            .is_err()
        {
            worker.abort_preparation();
            return Err("worker-ready-timeout");
        }
        let status = worker.status();
        if status.phase != "ready-held"
            || !status.task_active
            || !status.native_live
            || status.native_lifecycles != Some(["issued", "issued"])
            || status.native_in_flight != Some([0, 0])
            || !status.progress_current
            || status.steps != 0
            || status.publication_sequence != 0
        {
            worker.abort_preparation();
            return Err("worker-ownership-unavailable");
        }
        Ok(())
    }
}
