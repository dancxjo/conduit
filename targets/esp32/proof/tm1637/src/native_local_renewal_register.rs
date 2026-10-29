//! Re-register only the actual returned executor slots with independently
//! prepared owners. Interrupted and normally completed Mask slots stay distinct.
use super::*;
impl NativeRoot {
    pub(super) async fn register_returned_local(
        &mut self,
        state: &mut local_renewal::Renewal,
    ) -> Result<(), &'static str> {
        let spawner = self.worker_spawner.ok_or("renewal-worker-executor")?;
        if self.local_mask_worker.is_none() {
            let old = state
                .mask_handle
                .take()
                .ok_or("renewal-retired-mask-handle")?;
            let result = match state.mask_interrupted {
                false => old.prepare_next(
                    &mut state.fresh_session,
                    &mut state.fresh_leases,
                    &mut state.fresh_evidence,
                    spawner,
                ),
                true => old.prepare_after_interruption(
                    &mut state.fresh_session,
                    &mut state.fresh_leases,
                    &mut state.fresh_evidence,
                    spawner,
                ),
            };
            match result {
                Ok(handle) => self.local_mask_worker = Some(handle),
                Err((handle, reason)) => {
                    state.mask_handle = Some(handle);
                    return Err(reason);
                }
            }
            self.display.mark_local_task_registered()?;
        }
        let mask = self
            .local_mask_worker
            .as_ref()
            .expect("retained next Mask task");
        embassy_time::with_timeout(embassy_time::Duration::from_secs(2), mask.wait_ready())
            .await
            .map_err(|_| "renewal-mask-ready-timeout")?;
        if mask.status().phase != "ready-held" {
            return Err("renewal-mask-not-held");
        }
        self.prepare_returned_sampler_worker(
            &mut state.fresh_kernel,
            &mut state.fresh_native,
            &mut state.registered_publisher,
            state.fresh_active.as_ref().ok_or("renewal-fresh-active")?,
            state
                .fresh_fragment
                .as_ref()
                .ok_or("renewal-fresh-fragment")?,
            &mut state.sampler_handle,
        )
        .await
    }
}
