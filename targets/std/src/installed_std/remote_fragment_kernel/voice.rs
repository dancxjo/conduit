//! Completion of exact admitted voice provider calls for a remote fragment.
use super::super::{model_host, whisper_language, whisper_speech_back};
use super::*;

impl InstalledRemoteFragment {
    pub(crate) fn complete_voice_provider_host_call<F>(
        &mut self,
        request: HostCallRequest,
        speech_recognition: Option<&mut crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        mut local_model: Option<
            &mut (dyn crate::hosted_local_model::HostedLocalModelAdapter + 'static),
        >,
        cancelled: F,
    ) -> Result<bool, String>
    where
        F: Fn() -> bool + Copy,
    {
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|operation| operation.node == request.node && operation.call == request.call)
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let contract = operation.contract_id.as_str();
        let maximum_output_bytes = operation.binding.maximum_output_bytes;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote voice host input: {error:?}"))?;
        let outcome = if matches!(
            contract,
            conduit_std_offers::WHISPER_SPEECH_OPERATION
                | conduit_std_offers::WHISPER_CLIP_SPEECH_OPERATION
        ) {
            let recognition = whisper_language::execute(
                speech_recognition,
                self.whisper_languages.get(request.node)?,
                contract,
                input,
                cancelled,
            );
            match recognition {
                Ok(encoded) => {
                    let value = self
                        .scheduler
                        .store_host_value(&encoded)
                        .map_err(|error| format!("store remote Whisper recognition: {error:?}"))?;
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, maximum_output_bytes).map_err(
                            |error| format!("bound remote Whisper recognition: {error:?}"),
                        )?),
                        failure: None,
                    }
                }
                Err(failure) => whisper_speech_back::failure_outcome(failure),
            }
        } else if contract == conduit_ai::LOCAL_MODEL_OPERATION {
            let placement = self
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "remote model request has no exact placement".to_string())?;
            let completion = if cancelled() {
                if let Some(adapter) = local_model.as_mut() {
                    (*adapter).cancel_stream();
                }
                model_host::ModelHostCompletion::Cancelled
            } else {
                let completion = model_host::execute(
                    contract,
                    placement,
                    input,
                    local_model.as_mut().map(|adapter| {
                        &mut **adapter
                            as &mut (dyn crate::hosted_local_model::HostedLocalModelAdapter
                                      + 'static)
                    }),
                    &mut self.model_output_buffer,
                )?;
                if cancelled() {
                    if let Some(adapter) = local_model.as_mut() {
                        (*adapter).cancel_stream();
                    }
                    model_host::ModelHostCompletion::Cancelled
                } else {
                    completion
                }
            };
            let output = if completion.has_output() {
                let value = self
                    .scheduler
                    .store_host_value(&self.model_output_buffer)
                    .map_err(|error| format!("store remote model output: {error:?}"))?;
                Some(
                    BoundedValueRef::new(value, maximum_output_bytes)
                        .map_err(|error| format!("bound remote model output: {error:?}"))?,
                )
            } else {
                None
            };
            completion.outcome(output)
        } else {
            return Ok(false);
        };
        self.complete_host_call(request, outcome)?;
        Ok(true)
    }
}
