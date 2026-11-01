//! Completion of the existing microphone operation on an admitted remote fragment.
use super::super::microphone_host;
use super::*;

impl InstalledRemoteFragment {
    pub(crate) fn prepare_microphone_flow_host(
        &mut self,
        fragment: &conduit_core::PlanFragment,
        adapter: Option<&crate::hosted_microphone::AlsaMicrophoneAdapter>,
    ) -> Result<(), String> {
        self.microphone_flow_host = Some(
            super::super::microphone_flow_host::MicrophoneFlowHost::prepare(fragment, adapter)?,
        );
        Ok(())
    }
    pub(crate) fn complete_microphone_provider_host_call(
        &mut self,
        request: HostCallRequest,
        microphone: Option<&mut crate::hosted_microphone::AlsaMicrophoneAdapter>,
        cancelled: impl FnMut() -> bool,
    ) -> Result<bool, String> {
        let work = self.host_request_view(request)?;
        if work.contract_id.as_str() == conduit_std_offers::MICROPHONE_FLOW_OPERATION {
            let host = self
                .microphone_flow_host
                .as_mut()
                .ok_or("finite capture lacks a prepared provider lease")?;
            let mut cancelled = cancelled;
            host.accept(request, &mut self.scheduler, cancelled())?;
            return Ok(true);
        }
        if work.contract_id.as_str() != conduit_std_offers::MICROPHONE_CLIP_OPERATION {
            return Ok(false);
        }
        let maximum_output_bytes = work.maximum_output_bytes;
        let placement = self
            .placements
            .get(usize::from(request.node.0))
            .ok_or("remote microphone request has no exact placement")?;
        let capture = microphone_host::execute(
            work.input,
            placement.implementation_id.as_str(),
            microphone,
            cancelled,
        )?;
        let outcome = match capture {
            Ok(clip) => {
                if clip.len() > maximum_output_bytes as usize {
                    return Err("captured microphone clip exceeds admitted output bound".into());
                }
                let value = self
                    .scheduler
                    .store_host_value(&clip)
                    .map_err(|error| format!("store remote microphone clip: {error:?}"))?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, maximum_output_bytes)
                            .map_err(|error| format!("bound remote microphone clip: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
            Err(error) => microphone_host::failure_outcome(error),
        };
        self.complete_host_call(request, outcome)?;
        Ok(true)
    }
}
