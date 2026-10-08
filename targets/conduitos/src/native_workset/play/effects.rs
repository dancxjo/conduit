//! Exact typed Host completions; retained editing uses portable semantics.
use super::*;
use conduit_human::KeymapDisposition;
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
                    KeymapDisposition::Text(text) => {
                        let mut utf8 = [0; 4];
                        self.output(request, Some(text.encode_utf8(&mut utf8)))
                    }
                    KeymapDisposition::NoText | KeymapDisposition::Cancelled => {
                        self.output(request, None)
                    }
                    KeymapDisposition::Refused(_) => {
                        self.failed(request, FailureCode::InvalidInput, 72)
                    }
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
                    self.output(request, Some(result.upper()))
                }
                #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
                {
                    let text =
                        crate::text_upper::uppercase(input).map_err(|_| PlayRefusal::Kernel)?;
                    self.output(request, Some(text.as_bytes()))
                }
            }
            Effect::Edit => {
                let output = match self.editors[plot]
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
                if self.presentations[plot].is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                #[cfg(all(target_arch = "x86_64", target_os = "none"))]
                if let Some(domain) = &mut self.protected[plot] {
                    if let Err(error) = domain.present(input, &mut crate::arch::Serial::new()) {
                        return self.protected_failure(request, error);
                    }
                }
                self.output(request, None)?;
                if self.presentations[plot].replace(text).is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                Ok(())
            }
            Effect::Application => {
                let application = self.applications[plot]
                    .as_mut()
                    .ok_or(PlayRefusal::Kernel)?;
                let (view, authority_request) = match application {
                    NativeApplication::Tutorial(application) => application.apply(input)?,
                    NativeApplication::Tour(application) => {
                        let output = application.apply(input).map_err(|_| PlayRefusal::Kernel)?;
                        let request = match output.request {
                            Some(conduit_tour_model::TourWorkspaceRequest::Run {
                                chapter,
                                stage,
                            }) => Some(super::super::NativeApplicationRequest::RunTour {
                                chapter,
                                stage,
                            }),
                            Some(conduit_tour_model::TourWorkspaceRequest::OpenPatchbay) => {
                                Some(super::super::NativeApplicationRequest::OpenPatchbay)
                            }
                            None => None,
                        };
                        (output.view, request)
                    }
                    NativeApplication::Patchbay(application) => {
                        let output = application.apply(input)?;
                        (
                            output.view,
                            output
                                .request
                                .map(super::super::NativeApplicationRequest::EditCurrent),
                        )
                    }
                };
                if authority_request.is_some() && self.application_requests[plot].is_some() {
                    return Err(PlayRefusal::InputPressure);
                }
                self.application_requests[plot] = authority_request;
                self.output(request, Some(&view))
            }
            Effect::ApplicationPresentation => {
                let view = conduit_presentation::ApplicationView::decode(input)
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
    pub(super) fn output(
        &mut self,
        request: HostCallRequest,
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
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(Failure { code, detail }),
                },
            )
            .map_err(|_| PlayRefusal::Kernel)
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
