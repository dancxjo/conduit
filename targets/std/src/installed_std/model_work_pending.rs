//! One ordinary scheduler request owns the already prepared model adapter.
//! This holds no second queue, scheduler, provider, or borrowed input value.
use super::{
    model_work_host::{self, Completion},
    InstalledScheduler,
};
use crate::hosted_model_work::HostedModelWorkAdapter;
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{HostCallCancellation, HostCallRequest},
    BoundedValueRef,
};

#[derive(Clone, Copy)]
struct Pending {
    request: HostCallRequest,
    maximum_output_bytes: u32,
}

pub(super) struct InstalledModelWorkHost<'a> {
    adapter: Option<&'a mut (dyn HostedModelWorkAdapter + 'static)>,
    pending: Option<Pending>,
    last_request: Option<HostCallRequest>,
}

impl<'a> InstalledModelWorkHost<'a> {
    pub(super) fn new(adapter: Option<&'a mut (dyn HostedModelWorkAdapter + 'static)>) -> Self {
        Self {
            adapter,
            pending: None,
            last_request: None,
        }
    }
    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn execute(
        &mut self,
        request: HostCallRequest,
        maximum_output_bytes: u32,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<Option<Completion>, String> {
        output.clear();
        if self.pending.is_some() {
            // The kernel retains the one admitted in-flight input. A second
            // dispatched request cannot overwrite that owner or grow a queue.
            return Ok(Some(Completion::Refused));
        }
        let completion =
            model_work_host::execute(placement, input, self.adapter.as_deref_mut(), output)?;
        self.last_request = Some(request);
        if completion.is_none() {
            output.clear();
            self.pending = Some(Pending {
                request,
                maximum_output_bytes,
            });
        }
        Ok(completion)
    }

    pub(super) fn cancel(&mut self, cancellation: HostCallCancellation) {
        if !self
            .last_request
            .is_some_and(|request| matches(request, cancellation))
        {
            return;
        }
        // Revoke host publication before entering provider cleanup. Kernel
        // cancellation already owns its terminal outcome; do not complete an
        // already cancelled scheduler request a second time.
        self.pending = None;
        self.last_request = None;
        if let Some(adapter) = self.adapter.as_deref_mut() {
            adapter.cancel();
        }
    }

    pub(super) fn poll(
        &mut self,
        scheduler: &mut InstalledScheduler,
        output: &mut Vec<u8>,
        stop_requested: impl Fn() -> bool,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending else {
            return Ok(false);
        };
        if stop_requested() {
            return Ok(false);
        }
        output.clear();
        let completion = match self.adapter.as_deref_mut() {
            Some(adapter) => model_work_host::completion(adapter.poll(output)),
            None => Some(Completion::ProviderLost),
        };
        let Some(completion) = completion else {
            output.clear();
            return Ok(false);
        };
        // A newly requested stop returns to the existing cancellation owner
        // before any output is stored. Accepted kernel cancellation is drained
        // before this method is called; completion linearizes in that kernel.
        if stop_requested() {
            output.clear();
            return Ok(false);
        }
        let value = if completion.has_output() {
            if output.len() > pending.maximum_output_bytes as usize {
                self.pending = None;
                if let Some(adapter) = self.adapter.as_deref_mut() {
                    adapter.cancel();
                }
                output.clear();
                return scheduler
                    .complete_host_call(
                        pending.request.node,
                        pending.request.request,
                        Completion::Failed.outcome(None),
                    )
                    .map(|()| true)
                    .map_err(|error| {
                        format!("complete oversized pending model output: {error:?}")
                    });
            }
            let value = scheduler
                .store_host_value(output)
                .map_err(|error| format!("store pending model-work output: {error:?}"))?;
            Some(
                BoundedValueRef::new(value, pending.maximum_output_bytes)
                    .map_err(|error| format!("bound pending model-work output: {error:?}"))?,
            )
        } else {
            output.clear();
            None
        };
        scheduler
            .complete_host_call(
                pending.request.node,
                pending.request.request,
                completion.outcome(value),
            )
            .map_err(|error| format!("complete pending model-work operation: {error:?}"))?;
        self.pending = None;
        Ok(true)
    }
}

impl Drop for InstalledModelWorkHost<'_> {
    fn drop(&mut self) {
        if self.pending.take().is_some() {
            // Errors/early return cannot strand a retained worker with a live
            // result. Completed runs retain their already warmed preparation.
            if let Some(adapter) = self.adapter.as_deref_mut() {
                adapter.cancel();
                adapter.unload();
            }
        }
    }
}

fn matches(request: HostCallRequest, cancellation: HostCallCancellation) -> bool {
    request.node == cancellation.node
        && request.request == cancellation.request
        && request.call == cancellation.call
}

#[cfg(test)]
#[path = "model_work_pending_tests.rs"]
mod tests;
