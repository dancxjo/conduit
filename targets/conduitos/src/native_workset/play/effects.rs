//! Exact typed Host completions; retained editing uses portable semantics.
use super::*;

use conduit_kernel::{BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome};

impl NativeWorksetPlay {
    pub(super) fn apply(
        &mut self,
        request: HostCallRequest,
        binding: Binding,
    ) -> Result<(), PlayRefusal> {
        let plot = usize::from(binding.plot);
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| PlayRefusal::Kernel)?;
        match binding.effect {
            Effect::Keymap => {
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                if let Some(domain) = &mut self.protected[plot] {
                    if self.pure_results[plot].is_some() {
                        return Err(PlayRefusal::InputPressure);
                    }
                    let result = match domain.keymap_chain(input) {
                        Ok(result) => result,
                        Err(crate::text_protection::KeyboardChainError::InputRefused) => {
                            return self.failed(request, FailureCode::InvalidInput, 72);
                        }
                        Err(crate::text_protection::KeyboardChainError::Execution(error)) => {
                            return self.protected_failure(request, error);
                        }
                    };
                    self.output(request, result.as_ref().map(|result| result.source()))?;
                    self.pure_results[plot] = result;
                    return Ok(());
                }
                let event = KeyEvent::decode(input).map_err(|_| PlayRefusal::Kernel)?;
                match self.keymaps[plot].apply(event) {
                    Ok(Some(text)) => self.output(request, Some(text.as_slice())),
                    Ok(None) => self.output(request, None),
                    Err(_) => self.failed(request, FailureCode::InvalidInput, 72),
                }
            }
            Effect::Upper => {
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                {
                    let Some(result) = self.pure_results[plot].take() else {
                        return self.protected_failure(
                            request,
                            crate::composition::MachineRunError::KernelFailure,
                        );
                    };
                    if result.source() != input {
                        return self.protected_failure(
                            request,
                            crate::composition::MachineRunError::KernelFailure,
                        );
                    }
                    // This is the result already computed in the domain. The
                    // existing kernel retains the original typed Cord flow.
                    self.output(request, Some(result.transformed()))
                }
                #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
                {
                    let text =
                        crate::text_upper::uppercase(input).map_err(|_| PlayRefusal::Kernel)?;
                    self.output(request, Some(text.as_bytes()))
                }
            }
            Effect::Edit => {
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                {
                    let Some(result) = self.pure_results[plot].take() else {
                        return self.protected_failure(
                            request,
                            crate::composition::MachineRunError::KernelFailure,
                        );
                    };
                    if result.source() != input {
                        return self.protected_failure(
                            request,
                            crate::composition::MachineRunError::KernelFailure,
                        );
                    }
                    if result.edit_refused() {
                        return self.failed(request, FailureCode::StateCapacityExhausted, 82);
                    }
                    return self.output(request, Some(result.transformed()));
                }
                #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
                {
                    let output = match self.editors[plot]
                        .as_mut()
                        .ok_or(PlayRefusal::Kernel)?
                        .apply(input)
                    {
                        Ok(Some(text)) => Some(NativeFace::new(text)?),
                        Ok(None) => None,
                        Err(conduit_semantic_catalog::TextStateRefusal::CapacityExhausted) => {
                            return self.failed(request, FailureCode::StateCapacityExhausted, 82);
                        }
                        Err(_) => return self.failed(request, FailureCode::InvalidInput, 83),
                    };
                    self.output(request, output.as_ref().map(|text| text.text().as_bytes()))
                }
            }
            Effect::Document => {
                let edit = match conduit_semantic_catalog::TextEdit::decode(input) {
                    Ok(edit) => edit,
                    Err(refusal) => return self.document_refused(request, refusal),
                };
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                {
                    let domain = self.protected[plot]
                        .as_mut()
                        .ok_or(PlayRefusal::Preparation)?;
                    match domain.replace_document(edit) {
                        Ok(observed) => self.output(request, Some(observed.as_bytes())),
                        Err(crate::text_protection::ProtectedDocumentError::Refused(refusal)) => {
                            self.document_refused(request, refusal)
                        }
                        Err(crate::text_protection::ProtectedDocumentError::Execution(error)) => {
                            self.protected_failure(request, error)
                        }
                    }
                }
                #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
                {
                    let owner = self.editors[plot]
                        .as_mut()
                        .ok_or(PlayRefusal::Preparation)?;
                    if let Err(refusal) = owner.replace(edit) {
                        return self.document_refused(request, refusal);
                    }
                    let mut bytes =
                        [0; conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES as usize];
                    let length = conduit_semantic_catalog::TextDocument {
                        revision: owner.revision(),
                        text: owner.current(),
                    }
                    .encode_into(&mut bytes)
                    .map_err(|_| PlayRefusal::Kernel)?;
                    self.output(request, Some(&bytes[..length]))
                }
            }
            Effect::Face => {
                let text = NativeFace::new(input)?;
                if self.faces[plot].is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                if let Some(domain) = &mut self.protected[plot] {
                    if let Err(error) = domain.present(input, &mut crate::arch::Serial::new()) {
                        return self.protected_failure(request, error);
                    }
                }
                self.output(request, None)?;
                if self.faces[plot].replace(text).is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                Ok(())
            }
            Effect::Application => {
                // Reserve owner-request storage before the protected actor mutates.
                if self.application_requests[plot].is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                let result = self.applications.apply(plot, input);
                let (snapshot, authority_request) = self.application_result(result)?;
                let view = snapshot.view;
                self.output(request, Some(&view))?;
                self.application_requests[plot] = authority_request;
                Ok(())
            }
            Effect::ApplicationFace => {
                let view = conduit_face::ApplicationView::decode(input)
                    .map_err(|_| PlayRefusal::Kernel)?;
                self.output(request, None)?;
                if self.application_views[plot].replace(view).is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                Ok(())
            }
            Effect::Keyboard | Effect::ApplicationEvent => Err(PlayRefusal::Kernel),
        }
    }
    fn document_refused(
        &mut self,
        request: HostCallRequest,
        refusal: conduit_semantic_catalog::TextEditRefusal,
    ) -> Result<(), PlayRefusal> {
        let (code, detail) = super::super::document_state::failure(refusal);
        self.failed(request, code, detail)
    }
    pub(super) fn output(
        &mut self,
        request: HostCallRequest,
        bytes: Option<&[u8]>,
    ) -> Result<(), PlayRefusal> {
        let result = self.output_inner(request, bytes);
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if result.is_err() {
            self.fence_runtime_failure();
        }
        result
    }
    fn output_inner(
        &mut self,
        request: HostCallRequest,
        bytes: Option<&[u8]>,
    ) -> Result<(), PlayRefusal> {
        if self.cancelled || !self.applications_live() {
            return Err(PlayRefusal::Cancelled);
        }
        let output = bytes
            .map(|bytes| {
                let value = self
                    .scheduler
                    .store_host_value(bytes)
                    .map_err(PlayRefusal::Scheduler)?;
                // Empty text is still a value on the flow. Its admitted output
                // envelope stays nonzero, as required by the kernel boundary.
                BoundedValueRef::new(value, (bytes.len() as u32).max(1))
                    .map_err(|_| PlayRefusal::Kernel)
            })
            .transpose()?;
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output,
                    failure: None,
                },
            )
            .map_err(PlayRefusal::Scheduler)
    }
    fn failed(
        &mut self,
        request: HostCallRequest,
        code: FailureCode,
        detail: u16,
    ) -> Result<(), PlayRefusal> {
        self.complete_failure(request, code, detail)?;
        Err(PlayRefusal::HostFailure(Failure { code, detail }))
    }

    pub(super) fn complete_failure(
        &mut self,
        request: HostCallRequest,
        code: FailureCode,
        detail: u16,
    ) -> Result<(), PlayRefusal> {
        let result = self
            .scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(Failure { code, detail }),
                },
            )
            .map_err(|_| PlayRefusal::Kernel);
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if result.is_err() {
            self.fence_runtime_failure();
        }
        result
    }

    pub fn input_lost(&mut self) -> Result<(), PlayRefusal> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        self.revoke_protection(crate::protection_domain::KernelRevocationCause::ProviderLost);
        for request in self.pending.into_iter().flatten() {
            self.complete_failure(request, FailureCode::HostCallFailed, 81)?;
        }
        self.cancel()
    }
}
