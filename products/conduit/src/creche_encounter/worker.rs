//! Admission and lifetime custody of one finite native Crèche production worker.
use super::*;
use conduit_std_host::{body_execution::ImageMemoryRetention, StdHost};
use std::sync::atomic::Ordering;

/// Independently qualified complete worker highwater, including stack, heap,
/// runtime overhead and retained output backing. Descriptive bounds grant no IO.
#[derive(Clone, Copy, Debug)]
pub struct CrecheWorkerBounds {
    pub memory_bytes: usize,
    pub stack_bytes: usize,
}

pub(super) struct PreparedWorkerMemory {
    stack_bytes: usize,
}

/// Produced Seed data keeps its original worker charge through handoff.
/// Borrowing does not permit uncharged owned-data extraction or acknowledge
/// deployment, joining or durable recording.
pub struct ImageCrecheProducts {
    pub(super) products: ProducedJoiningHost<File>,
    _memory: ImageMemoryRetention,
}
impl ImageCrecheProducts {
    pub fn products(&self) -> &ProducedJoiningHost<File> {
        &self.products
    }
}

impl CrecheEncounterController {
    /// Reserve before activation/spawn on the actual producing Host counter.
    /// A second admission cannot replace or enlarge this worker's grant.
    ///
    /// # Safety
    /// Root must qualify the complete worker lifetime highwater, including OS
    /// stack rounding/guards, TLS/runtime overhead, all heap/output capacities,
    /// error/unwind paths and destruction. Existing inputs need independent
    /// custody. These cooperative bounds are not process or allocator limits,
    /// execution authority, or permission to open any unselected file/device.
    pub unsafe fn prepare_image_worker(
        &self,
        host: &mut StdHost,
        bounds: CrecheWorkerBounds,
    ) -> Result<(), String> {
        self.channel.verify_image(host)?;
        let mut state = self.channel.lock()?;
        if state.active
            || state.retired
            || state.started
            || state.cancelled.load(Ordering::Acquire)
            || state.worker_memory.is_some()
        {
            return Err("Crèche worker memory requires one inactive initial admission".into());
        }
        if !(64 * 1024..=32 * 1024 * 1024).contains(&bounds.stack_bytes)
            || !bounds.stack_bytes.is_multiple_of(4096)
            || bounds.memory_bytes
                <= bounds
                    .stack_bytes
                    .saturating_add(core::mem::size_of::<PreparedWorkerMemory>())
        {
            return Err(
                "Crèche worker requires a finite stack and complete memory envelope".into(),
            );
        }
        // SAFETY: caller qualifies the full asynchronous worker and result
        // lifetime. State retains this guard until join and every output handoff;
        // output witnesses keep its original charge after State destruction.
        state.worker_memory = Some(unsafe {
            host.prepare_selected_image_value(bounds.memory_bytes, bounds.memory_bytes, || {
                Ok(PreparedWorkerMemory {
                    stack_bytes: bounds.stack_bytes,
                })
            })
        }?);
        Ok(())
    }

    /// Service-side effect pump, never called from kernel begin/poll. Starts
    /// exactly once after canonical checking; retries cannot repeat production.
    pub fn start_production_if_reviewed(&self) -> Result<bool, String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.retired || state.started {
            return Ok(false);
        }
        if self.channel.is_image() && state.checked.is_some() && state.worker_memory.is_none() {
            return Err(
                "Crèche IMAGE production requires separately admitted worker memory".into(),
            );
        }
        let Some(configuration) = state.checked.take() else {
            return Ok(false);
        };
        let request = state
            .request
            .ok_or("Crèche reviewed operation lacks request")?;
        state.started = true;
        let cancelled = state.cancelled.clone();
        let channel = self.channel.clone();
        let mut builder = std::thread::Builder::new().name("conduit-creche-make".into());
        if let Some(memory) = &state.worker_memory {
            builder = builder.stack_size(memory.value().stack_bytes);
        }
        let worker = builder.spawn(move || {
            #[cfg(test)]
            let allocation_probe = crate::allocation_probe::begin();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let artifact = channel
                    .inputs
                    .artifacts
                    .iter()
                    .find(|a| {
                        a.configuration.configuration_id() == configuration.configuration_id()
                            || a.bundle
                                .as_ref()
                                .is_some_and(|bundle| bundle.admits_native_target(&configuration))
                    })
                    .ok_or("no owner-reviewed artifact matches this checked Host")?;
                if let Some(bundle) = &artifact.bundle {
                    return bundle.produce(
                        artifact,
                        &configuration,
                        &channel.binding,
                        &channel.inputs,
                        request,
                        &cancelled,
                    );
                }
                let file = artifact
                    .artifact
                    .try_clone()
                    .map_err(|e| format!("Crèche retain artifact handle: {e}"))?;
                let id = format!("creche/request/{:x}", Sha256::digest(request.0));
                let clock_failed = std::sync::atomic::AtomicBool::new(false);
                conduit_body_make::produce_joining_host(
                    conduit_body_make::JoiningProductionRequest {
                        request_id: &id,
                        body: &channel.binding.biography,
                        invitation: &channel.binding.invitation.claim,
                        configuration: &configuration,
                        output: artifact.output.clone(),
                        artifact: conduit_body_make::ReviewedJoiningArtifact {
                            image: &artifact.image,
                            image_manifest: &artifact.image_manifest,
                            artifact_digest: &artifact.artifact_digest,
                            artifact_bytes: artifact.artifact_bytes,
                            source_identity: &artifact.source_identity,
                        },
                        bounds: channel.inputs.installation.production_bounds,
                    },
                    file,
                    &channel.inputs.catalog,
                    &channel.inputs.packages,
                    || match clock() {
                        Ok(now) => now,
                        Err(_) => {
                            clock_failed.store(true, Ordering::Release);
                            u64::MAX
                        }
                    },
                    || cancelled.load(Ordering::Acquire) || clock_failed.load(Ordering::Acquire),
                )
            }))
            .unwrap_or_else(|_| Err("Crèche production worker panicked; drain required".into()));
            #[cfg(test)]
            eprintln!(
                "Crèche worker requested allocation diagnostic (image={}): {:?}",
                channel.is_image(),
                allocation_probe.finish_statistics()
            );
            // One worker owns the result slot. Keep custody even after a mutex
            // poison; the adapter will refuse further work and drain explicitly.
            channel
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .result = Some(result);
        });
        match worker {
            Ok(worker) => {
                state.worker = Some(worker);
                Ok(true)
            }
            Err(error) => {
                state.result = Some(Err(format!("Crèche production launch: {error}")));
                Ok(true)
            }
        }
    }

    /// Transfer only actually published Seed data, retaining its worker charge.
    pub fn take_image_products(&self) -> Result<Option<ImageCrecheProducts>, String> {
        self.channel.verify_image_profile()?;
        let mut state = self.channel.lock()?;
        if !state.published || state.products.is_none() {
            return Ok(None);
        }
        let memory = state
            .worker_memory
            .as_ref()
            .ok_or("Crèche published IMAGE products lack admitted worker custody")?
            .retain_memory();
        Ok(state.products.take().map(|products| ImageCrecheProducts {
            products,
            _memory: memory,
        }))
    }
}
