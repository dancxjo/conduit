//! Exact admitted Host Call dispatch for local Body Play.
use super::super as installed;
use super::*;

impl BodyKernel<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch_host_calls<W: Write, T: TimerAdapter>(
        &mut self,
        keys: &mut installed::keyboard_input_host::KeyboardInputHost<'_>,
        deadlines: &mut installed::deadline_host::InstalledDeadlineHost<PENDING_REQUESTS>,
        clock: &mut T,
        output: &mut W,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        mut creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
    ) -> Result<(), String> {
        while let Some(request) = self.scheduler.next_host_request() {
            self.clock_observations.capture_new(
                self.scheduler.signs().events(),
                clock,
                host_id,
                boot_id,
            );
            if !self
                .requests
                .iter()
                .any(|observed| observed.node == request.node && observed.call == request.call)
            {
                if self.requests.len() == self.requests.capacity() {
                    return Err("Body observed-operation capacity exceeded".into());
                }
                self.requests.push(request);
            }
            let operation = self
                .operations
                .iter()
                .find(|op| op.node == request.node && op.call == request.call)
                .ok_or("Body request has no exact partition operation")?;
            if operation.contract_id.as_str()
                == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
            {
                self.begin_creche(request, creche.as_deref_mut())?;
                continue;
            }
            let input = self
                .scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("Body request value: {error:?}"))?;
            if operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL {
                let checkpoint = self
                    .todo_checkpoint
                    .as_ref()
                    .ok_or("Todo checkpoint Host Call has no selected residence")?;
                let outcome = checkpoint.perform(request, input);
                self.scheduler
                    .complete_host_call(request.node, request.request, outcome)
                    .map_err(|error| format!("Todo checkpoint completion: {error:?}"))?;
                continue;
            }
            if operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL {
                let reader = self
                    .todo_checkpoint_read
                    .as_ref()
                    .ok_or("Todo checkpoint read Host Call has no selected residence")?;
                let result = reader.read(request, input);
                let outcome = match result {
                    Ok(bytes) => {
                        let value = self.scheduler.store_host_value(&bytes).map_err(|error| {
                            format!("Todo checkpoint read value storage: {error:?}")
                        })?;
                        let output =
                            BoundedValueRef::new(value, conduit_todo_plot::STATE_MAX_BYTES as u32)
                                .map_err(|error| {
                                    format!("Todo checkpoint read bound: {error:?}")
                                })?;
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: Some(output),
                            failure: None,
                        }
                    }
                    Err(error) => HostCallOutcome {
                        disposition: if error
                            == crate::todo_durable_resource::Refusal::InvalidBinding
                        {
                            HostCallDisposition::Denied
                        } else {
                            HostCallDisposition::Failed
                        },
                        output: None,
                        failure: Some(Failure {
                            code: if error == crate::todo_durable_resource::Refusal::InvalidBinding
                            {
                                FailureCode::HostCallDenied
                            } else {
                                FailureCode::HostCallFailed
                            },
                            detail: crate::todo_checkpoint_read_call::read_failure_detail(&error),
                        }),
                    },
                };
                self.scheduler
                    .complete_host_call(request.node, request.request, outcome)
                    .map_err(|error| format!("Todo checkpoint read completion: {error:?}"))?;
                continue;
            }
            if keyboard(&operation.contract_id) {
                keys.accept(
                    request,
                    input,
                    installed::keyboard_input_host::InputRequestKind::Keyboard,
                )?;
                continue;
            }
            if button(&operation.contract_id) {
                keys.accept(
                    request,
                    input,
                    installed::keyboard_input_host::InputRequestKind::SpaceButton,
                )?;
                continue;
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
                continue;
            }
            if input_semantic(&operation.contract_id) {
                let node = usize::from(request.node.0);
                let completion = if operation.contract_id.as_str()
                    == conduit_std_offers::KEYMAP_SELECT_HOST_CALL
                {
                    installed::input_semantic_backs::select_host(
                        &mut self.input_keymaps[node],
                        input,
                    )
                } else {
                    installed::input_semantic_backs::execute_host(
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
                continue;
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
                continue;
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
                continue;
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
                continue;
            }
            if timer(&operation.contract_id) {
                let duration =
                    conduit_time::decode_tick(input).map_err(|error| error.to_string())?;
                let now = clock
                    .monotonic_now_ms()
                    .ok_or_else(|| "admitted monotonic wait Base is unavailable".to_string())?;
                deadlines.arm(request, duration, now)?;
                continue;
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
                .map_err(|error| format!("Body Host completion: {error:?}"))?;
        }
        Ok(())
    }
}
