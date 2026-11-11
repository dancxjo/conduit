//! One pending generation belongs to one existing kernel Host Call.
use super::{
    model_host::{self, ModelHostCompletion},
    InstalledScheduler,
};
use crate::hosted_local_model::HostedLocalModelAdapter;
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{HostCallCancellation, HostCallRequest},
    BoundedValueRef,
};

#[derive(Clone, Copy)]
struct Pending {
    request: HostCallRequest,
    maximum_output_bytes: u32,
    input_bytes: usize,
}
pub(super) struct InstalledLocalModelHost<'a> {
    adapter: Option<&'a mut (dyn HostedLocalModelAdapter + 'static)>,
    pending: Option<Pending>,
    play: String,
}
impl<'a> InstalledLocalModelHost<'a> {
    pub(super) fn prepare(
        mut adapter: Option<&'a mut (dyn HostedLocalModelAdapter + 'static)>,
        placements: &[PlannedGear],
        play: &str,
    ) -> Result<Self, String> {
        if let Some(adapter) = adapter.as_deref_mut() {
            adapter.prepare_background_generation(placements)?;
        }
        Ok(Self {
            adapter,
            pending: None,
            play: play.into(),
        })
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
    ) -> Result<Option<ModelHostCompletion>, String> {
        output.clear();
        if self.pending.is_some() {
            return Ok(Some(ModelHostCompletion::Refused));
        }
        if placement.kind_id.as_str() != conduit_ai::LLM_GENERATE_FLOW_KIND {
            return model_host::execute(
                conduit_ai::LOCAL_MODEL_OPERATION,
                placement,
                input,
                (&self.play, request.request),
                self.adapter.as_deref_mut(),
                output,
            )
            .map(Some);
        }
        super::local_model_back::validate(placement)?;
        let Some(adapter) = self.adapter.as_deref_mut() else {
            return Ok(Some(ModelHostCompletion::Refused));
        };
        if !adapter
            .offer()
            .capability_offers()
            .map_err(|e| format!("generation offer: {e:?}"))?
            .iter()
            .any(|offer| {
                offer.kind_id == placement.kind_id
                    && offer.implementation.artifact_id == placement.artifact_id
            })
        {
            return Ok(Some(ModelHostCompletion::Refused));
        }
        match adapter.begin_generation(placement, input, output) {
            Some(terminal) => model_host::finish_terminal(
                terminal,
                placement,
                (&self.play, request.request),
                input.len(),
                output,
            )
            .map(Some),
            None => {
                output.clear();
                self.pending = Some(Pending {
                    request,
                    maximum_output_bytes,
                    input_bytes: input.len(),
                });
                Ok(None)
            }
        }
    }
    pub(super) fn poll(
        &mut self,
        scheduler: &mut InstalledScheduler,
        placements: &[PlannedGear],
        output: &mut Vec<u8>,
        stopped: impl Fn() -> bool,
    ) -> Result<(), String> {
        let Some(pending) = self.pending else {
            return Ok(());
        };
        if stopped() {
            return Ok(());
        }
        output.clear();
        let Some(terminal) = self
            .adapter
            .as_deref_mut()
            .and_then(|a| a.poll_generation(output))
        else {
            return Ok(());
        };
        if stopped() {
            output.clear();
            return Ok(());
        }
        let placement = placements
            .get(usize::from(pending.request.node.0))
            .ok_or("pending generation placement missing")?;
        let completion = model_host::finish_terminal(
            terminal,
            placement,
            (&self.play, pending.request.request),
            pending.input_bytes,
            output,
        )?;
        let completion =
            if completion.has_output() && output.len() > pending.maximum_output_bytes as usize {
                output.clear();
                ModelHostCompletion::InvalidStructuredResult
            } else {
                completion
            };
        let value = if completion.has_output() {
            let value = scheduler
                .store_host_value(output)
                .map_err(|e| format!("store pending generation: {e:?}"))?;
            Some(
                BoundedValueRef::new(value, pending.maximum_output_bytes)
                    .map_err(|e| format!("bound pending generation: {e:?}"))?,
            )
        } else {
            None
        };
        scheduler
            .complete_host_call(
                pending.request.node,
                pending.request.request,
                completion.outcome(value),
            )
            .map_err(|e| format!("complete pending generation: {e:?}"))?;
        self.pending = None;
        Ok(())
    }
    pub(super) fn cancel(&mut self, cancelled: HostCallCancellation) {
        if self.pending.is_some_and(|pending| {
            pending.request.node == cancelled.node && pending.request.request == cancelled.request
        }) {
            self.cancel_all();
        }
    }
    pub(super) fn cancel_all(&mut self) {
        if self.pending.take().is_some() {
            if let Some(adapter) = self.adapter.as_deref_mut() {
                adapter.cancel_generation();
            }
        }
    }
}
impl Drop for InstalledLocalModelHost<'_> {
    fn drop(&mut self) {
        self.cancel_all();
    }
}
