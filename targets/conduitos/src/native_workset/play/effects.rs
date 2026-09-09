//! Exact typed Host completions; retained editing uses portable semantics.
use super::*;
use conduit_human::KeymapDisposition;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
};

impl NativeWorksetPlay {
    pub(super) fn apply(
        &mut self,
        request: HostOperationRequest,
        binding: Binding,
    ) -> Result<(), PlayRefusal> {
        let form = usize::from(binding.form);
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| PlayRefusal::Kernel)?;
        match binding.effect {
            Effect::Keymap => {
                let event = KeyEvent::decode(input).map_err(|_| PlayRefusal::Kernel)?;
                match self.keymaps[form].apply(event) {
                    KeymapDisposition::Text(text) => self.output(request, Some(text.as_bytes())),
                    KeymapDisposition::NoText | KeymapDisposition::Cancelled => {
                        self.output(request, None)
                    }
                    KeymapDisposition::Refused(_) => {
                        self.failed(request, FailureCode::InvalidInput, 72)
                    }
                }
            }
            Effect::Upper => {
                let text = crate::text_upper::uppercase(input).map_err(|_| PlayRefusal::Kernel)?;
                self.output(request, Some(text.as_bytes()))
            }
            Effect::Edit => {
                let output = match self.editors[form]
                    .as_mut()
                    .ok_or(PlayRefusal::Kernel)?
                    .apply(input)
                {
                    Ok(Some(text)) => Some(NativePresentation::new(text)?),
                    Ok(None) => None,
                    Err(conduit_semantic_catalog::TextStateRefusal::CapacityExhausted) => {
                        return self.failed(request, FailureCode::StateCapacityExhausted, 82);
                    }
                    Err(_) => return self.failed(request, FailureCode::InvalidInput, 83),
                };
                self.output(request, output.as_ref().map(|text| text.text().as_bytes()))
            }
            Effect::Presentation => {
                let text = NativePresentation::new(input)?;
                self.output(request, None)?;
                if self.presentations[form].replace(text).is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                Ok(())
            }
            Effect::Keyboard => Err(PlayRefusal::Kernel),
        }
    }
    pub(super) fn output(
        &mut self,
        request: HostOperationRequest,
        bytes: Option<&[u8]>,
    ) -> Result<(), PlayRefusal> {
        if self.cancelled {
            return Err(PlayRefusal::Cancelled);
        }
        let output = bytes
            .map(|bytes| {
                let value = self
                    .scheduler
                    .store_host_value(bytes)
                    .map_err(PlayRefusal::Scheduler)?;
                // Empty text is still a value on the Flow. Its admitted output
                // envelope stays nonzero, as required by the kernel boundary.
                BoundedValueRef::new(value, (bytes.len() as u32).max(1))
                    .map_err(|_| PlayRefusal::Kernel)
            })
            .transpose()?;
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output,
                    failure: None,
                },
            )
            .map_err(PlayRefusal::Scheduler)
    }
    fn failed(
        &mut self,
        request: HostOperationRequest,
        code: FailureCode,
        detail: u16,
    ) -> Result<(), PlayRefusal> {
        self.complete_failure(request, code, detail)?;
        Err(PlayRefusal::HostFailure(Failure { code, detail }))
    }

    fn complete_failure(
        &mut self,
        request: HostOperationRequest,
        code: FailureCode,
        detail: u16,
    ) -> Result<(), PlayRefusal> {
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure: Some(Failure { code, detail }),
                },
            )
            .map_err(|_| PlayRefusal::Kernel)
    }

    pub fn input_lost(&mut self) -> Result<(), PlayRefusal> {
        for request in self.pending.into_iter().flatten() {
            self.complete_failure(request, FailureCode::HostOperationFailed, 81)?;
        }
        self.cancel()
    }
}
