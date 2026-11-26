//! Exact installed Host Call dispatch, preserving the existing selected handlers.
use super::*;
mod file_effects;
impl BodyKernel<'_> {
    pub(super) fn perform_patchbay_request(
        &mut self,
        request: HostCallRequest,
        fore_output: &mut Option<&mut dyn crate::body_execution::BodyForeOutputAdapter>,
    ) -> Result<(), String> {
        let host = fore_output
            .as_mut()
            .and_then(|adapter| adapter.patchbay_interaction_host())
            .ok_or("Patchbay Host Call lost its admitted owner receiver")?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("Patchbay Host Call value: {error:?}"))?;
        let outcome = host.apply_interaction(input);
        self.scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|error| format!("Patchbay Host Call completion: {error:?}"))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn perform_request<W: Write, T: TimerAdapter>(
        &mut self,
        request: HostCallRequest,
        keys: &mut super::super::keyboard_input_host::KeyboardInputHost<'_>,
        deadlines: &mut super::super::deadline_host::InstalledDeadlineHost<PENDING_REQUESTS>,
        clock: &mut T,
        output: &mut W,
        fore_output: &mut Option<&mut dyn crate::body_execution::BodyForeOutputAdapter>,
        creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
        control: &RunControl,
        external_cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        if !self
            .requests
            .iter()
            .any(|observed| observed.node == request.node && observed.call == request.call)
        {
            if self.requests.len() == self.requests.capacity() {
                return Err(diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body observed-operation capacity exceeded"),
                ));
            }
            self.requests.push(request);
        }
        if self.begin_native_source(request)? {
            return Ok(());
        }
        let operation = self
            .operations
            .iter()
            .find(|op| op.node == request.node && op.call == request.call)
            .ok_or_else(|| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body request has no exact partition operation"),
                )
            })?;
        if operation.contract_id.as_str() == super::super::patchbay_interaction_factory::OPERATION {
            return self.perform_patchbay_request(request, fore_output);
        }
        if operation.contract_id.as_str()
            == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
        {
            self.begin_creche(request, creche)?;
            return Ok(());
        }
        if super::ambient_home::is_home_face_call(operation) {
            let route = self
                .home_face_routes
                .iter_mut()
                .find(|route| route.handles(request))
                .ok_or("Home Face call has no funded route")?;
            return route.dispatch(&mut self.scheduler, request);
        }
        if scalar_math::complete(operation, request, &mut self.scheduler)? {
            return Ok(());
        }
        if super::super::measurement_plot_back::complete(operation, request, &mut self.scheduler)? {
            return Ok(());
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body request value: {error:?}"),
                )
            })?;
        if operation.contract_id.as_str() == conduit_std_offers::STARTUP_CHIME_OPERATION {
            let Some(InstalledBack::StartupChime(back)) =
                self.scheduler.drivers().get(usize::from(request.node.0))
            else {
                return Err(diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("startup request belongs to different installed Back"),
                ));
            };
            let output = self
                .startup_outputs
                .iter_mut()
                .find(|(node, _)| *node == request.node)
                .map(|(_, output)| output)
                .ok_or("startup request has no prepared selected speaker")?;
            let receipt = self
                .startup_receipts
                .iter_mut()
                .find(|receipt| receipt.node == request.node)
                .ok_or("startup request has no admitted receipt slot")?;
            if receipt
                .request
                .is_some_and(|prior| prior.0 >= request.request.0)
            {
                return Err(diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("stale startup request cannot overwrite terminal evidence"),
                ));
            }
            let outcome = output.execute_controlled(input, back.pcm(), control, || {
                control.stop_requested() || external_cancel()
            });
            receipt.request = Some(request.request);
            receipt.disposition = Some(outcome.disposition);
            receipt.failure = outcome.failure;
            receipt.kernel_accepted = false;
            if outcome.disposition == HostCallDisposition::Cancelled
                && (control.stop_requested() || external_cancel())
            {
                return Ok(());
            }
            self.scheduler
                .complete_host_call(request.node, request.request, outcome)
                .map_err(|error| {
                    diagnostic::error(
                        &mut self.error_diagnostic,
                        format_args!("startup completion: {error:?}"),
                    )
                })?;
            receipt.kernel_accepted = true;
            return Ok(());
        }
        if file_effects::handles(&operation.contract_id) {
            return self.perform_file_request(request);
        }
        if keyboard(&operation.contract_id) {
            keys.accept(
                request,
                input,
                super::super::keyboard_input_host::InputRequestKind::Keyboard,
            )?;
            return Ok(());
        }
        if button(&operation.contract_id) {
            keys.accept(
                request,
                input,
                super::super::keyboard_input_host::InputRequestKind::SpaceButton,
            )?;
            return Ok(());
        }
        if text_state(&operation.contract_id) {
            let completion = self
                .text_state_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or("Body text state has no admitted host")?
                .execute(input);
            let (disposition, output, failure) = match completion {
                Ok(encoded) => {
                    let output = encoded
                        .map(|encoded| self.scheduler.store_host_value(encoded))
                        .transpose()
                        .map_err(|error| format!("Body text state output: {error:?}"))?
                        .map(|value| {
                            BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                        })
                        .transpose()
                        .map_err(|error| format!("Body text state output bound: {error:?}"))?;
                    (HostCallDisposition::Completed, output, None)
                }
                Err(refusal) => (HostCallDisposition::Failed, None, Some(refusal)),
            };
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition,
                        output,
                        failure,
                    },
                )
                .map_err(|error| format!("Body text state completion: {error:?}"))?;
            return Ok(());
        }
        if input_semantic(&operation.contract_id) {
            let node = usize::from(request.node.0);
            let completion =
                if operation.contract_id.as_str() == conduit_std_offers::KEYMAP_SELECT_HOST_CALL {
                    super::super::input_semantic_backs::select_host(
                        &mut self.input_keymaps[node],
                        input,
                    )
                } else {
                    super::super::input_semantic_backs::execute_host(
                        operation.contract_id.as_str() == conduit_std_offers::KEYMAP_HOST_CALL,
                        &mut self.input_keymaps[node],
                        input,
                    )
                };
            let outcome = match completion {
                Ok(Some(encoded)) => {
                    let value = self
                        .scheduler
                        .store_host_value(encoded.as_slice())
                        .map_err(|error| format!("Body input semantic output: {error:?}"))?;
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                                .map_err(|error| {
                                    format!("Body input semantic output bound: {error:?}")
                                })?,
                        ),
                        failure: None,
                    }
                }
                Ok(None) => HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
                Err(failure) => HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(failure),
                },
            };
            self.scheduler
                .complete_host_call(request.node, request.request, outcome)
                .map_err(|error| format!("Body input semantic completion: {error:?}"))?;
            return Ok(());
        }
        if typed_record_codec(&operation.contract_id) {
            let completion = self
                .typed_record_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or("Body typed-record codec has no admitted host")?
                .execute(input);
            let (disposition, output, failure) = match completion {
                Ok(encoded) => {
                    let value = self
                        .scheduler
                        .store_host_value(encoded)
                        .map_err(|error| format!("Body typed-record output: {error:?}"))?;
                    let output =
                        BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                            .map_err(|error| {
                                format!("Body typed-record output bound: {error:?}")
                            })?;
                    (HostCallDisposition::Completed, Some(output), None)
                }
                Err(detail) => (
                    HostCallDisposition::Failed,
                    None,
                    Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostCallFailed,
                        detail,
                    }),
                ),
            };
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition,
                        output,
                        failure,
                    },
                )
                .map_err(|error| format!("Body typed-record completion: {error:?}"))?;
            return Ok(());
        }
        if operation.contract_id.as_str() == conduit_std_offers::IMAGE_TEXT_RECORD_OPERATION {
            let completion = self
                .image_text_record_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or("Body image-text record operation has no admitted host")?
                .execute(input);
            let (disposition, output, failure) = match completion {
                Ok(encoded) => {
                    let value = self
                        .scheduler
                        .store_host_value(encoded)
                        .map_err(|error| format!("Body image-text record output: {error:?}"))?;
                    let output =
                        BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                            .map_err(|error| {
                                format!("Body image-text record output bound: {error:?}")
                            })?;
                    (HostCallDisposition::Completed, Some(output), None)
                }
                Err(_) => (
                    HostCallDisposition::Failed,
                    None,
                    Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostCallFailed,
                        detail: 1,
                    }),
                ),
            };
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition,
                        output,
                        failure,
                    },
                )
                .map_err(|error| format!("Body image-text record completion: {error:?}"))?;
            return Ok(());
        }
        if matches!(
            operation.contract_id.as_str(),
            conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION
                | conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION
        ) {
            let completion = self
                .image_text_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or("Body image-text operation has no admitted host")?
                .execute(operation.contract_id.as_str(), input);
            let (disposition, output, failure) = match completion {
                Ok(encoded) => {
                    let output = encoded
                        .map(|encoded| self.scheduler.store_host_value(encoded))
                        .transpose()
                        .map_err(|error| format!("Body image-text output: {error:?}"))?
                        .map(|value| {
                            BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                        })
                        .transpose()
                        .map_err(|error| format!("Body image-text output bound: {error:?}"))?;
                    (HostCallDisposition::Completed, output, None)
                }
                Err(_) => (
                    HostCallDisposition::Failed,
                    None,
                    Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostCallFailed,
                        detail: 1,
                    }),
                ),
            };
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition,
                        output,
                        failure,
                    },
                )
                .map_err(|error| format!("Body image-text completion: {error:?}"))?;
            return Ok(());
        }
        if timer(&operation.contract_id) {
            let duration = conduit_time::decode_tick(input).map_err(|error| {
                diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
            })?;
            let now = clock.monotonic_now_ms().ok_or_else(|| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("admitted monotonic wait Base is unavailable"),
                )
            })?;
            deadlines.arm(request, duration, now).map_err(|error| {
                diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
            })?;
            return Ok(());
        } else if !simple_face_host::present_named(
            operation.target_kind.as_ref(),
            input,
            output,
            self.face_labels[usize::from(request.node.0)].as_deref(),
        )? {
            return Err("Body face contract became unsupported".into());
        }
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body Host completion: {error:?}"),
                )
            })?;
        Ok(())
    }
}
