//! Selected file-effect completion beneath the existing Body dispatcher.
//! No provider discovery, authority issuance or additional effect policy.
use super::*;

pub(super) fn handles(contract: &conduit_core::HostCallContractId) -> bool {
    data_text::is_file(contract)
        || matches!(
            contract.as_str(),
            conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
                | conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        )
}

impl BodyKernel<'_> {
    pub(super) fn perform_file_request(&mut self, request: HostCallRequest) -> Result<(), String> {
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
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| {
                diagnostic::error(
                    &mut self.error_diagnostic,
                    format_args!("Body request value: {error:?}"),
                )
            })?;
        if data_text::is_file(&operation.contract_id) {
            let source = self
                .data_text_hosts
                .iter_mut()
                .find(|host| host.contains_node(request.node))
                .ok_or("Body file Text operation has no prepared selected provider")?;
            let direction = crate::installed_std::data_text_host::operation_for_contract(
                &operation.contract_id,
            )
            .ok_or("Body file Text operation is unknown")?;
            let completion = source.execute(request.node, direction, input);
            crate::installed_std::data_text_host::complete(
                &mut self.scheduler,
                operation,
                request,
                completion,
            )?;
            return Ok(());
        }
        if operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL {
            let checkpoint = self
                .todo_checkpoint
                .as_ref()
                .ok_or("Todo checkpoint Host Call has no selected residence")?;
            let outcome = checkpoint.perform(request, input);
            self.scheduler
                .complete_host_call(request.node, request.request, outcome)
                .map_err(|error| format!("Todo checkpoint completion: {error:?}"))?;
            return Ok(());
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
                            .map_err(|error| format!("Todo checkpoint read bound: {error:?}"))?;
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(output),
                        failure: None,
                    }
                }
                Err(error) => HostCallOutcome {
                    disposition: if error == crate::todo_durable_resource::Refusal::InvalidBinding {
                        HostCallDisposition::Denied
                    } else {
                        HostCallDisposition::Failed
                    },
                    output: None,
                    failure: Some(Failure {
                        code: if error == crate::todo_durable_resource::Refusal::InvalidBinding {
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
            return Ok(());
        }
        unreachable!("file dispatch is selected by its exact existing contract")
    }
}
