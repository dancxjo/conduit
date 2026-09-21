use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static LOCAL_MODEL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct LocalModelOperation {
    maximum_input_bytes: u32,
    pending: Option<RequestId>,
    next_request: u32,
    closed: bool,
    flow: bool,
    emitted: bool,
    stream: bool,
    stream_complete: bool,
    input: Option<conduit_kernel::ValueRef>,
}

impl<const PORTS: usize> StepBack<PORTS> for LocalModelOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.stream_complete || (self.emitted && !self.flow && !self.stream) || self.closed {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if self.stream {
                        let Some(value) = self.input else {
                            return step_fail(FailureCode::InvalidLifecycle, 7);
                        };
                        let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes)
                        else {
                            return step_fail(FailureCode::InvalidInput, 8);
                        };
                        let next_request = RequestId(self.next_request);
                        let Some(next) = self.next_request.checked_add(1) else {
                            return step_fail(FailureCode::IdentityCapacityExhausted, 8);
                        };
                        io.consume_host_completion()
                            .expect("observed streaming local-model completion");
                        io.send(PortId(0), output.value)
                            .expect("ready local-model chunk output");
                        io.request_host_call(next_request, HostCallId(0), input)
                            .expect("next streaming local-model Host Call");
                        self.next_request = next;
                        self.pending = Some(next_request);
                    } else {
                        io.consume_host_completion()
                            .expect("observed local-model completion");
                        io.send(PortId(0), output.value)
                            .expect("ready local-model output");
                        self.pending = None;
                        self.emitted = true;
                    }
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) if self.stream => {
                    io.consume_host_completion()
                        .expect("observed local-model stream completion");
                    self.pending = None;
                    if let Some(value) = self.input.take() {
                        io.discard(value)
                            .expect("finished local-model stream input");
                    }
                    self.stream_complete = true;
                    return StepOutcome::Complete;
                }
                (HostCallDisposition::Denied, _, _) => {
                    return step_fail(FailureCode::HostCallDenied, 2)
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return step_fail(FailureCode::Cancelled, 3)
                }
                (HostCallDisposition::Failed, _, _) => {
                    return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: FailureCode::HostCallFailed,
                        detail: 4,
                    }))
                }
                _ => return step_fail(FailureCode::InvalidLifecycle, 5),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some()
                || self.closed
                || (self.stream && self.input.is_some())
                || (!self.stream && !self.flow && self.emitted)
            {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::IdentityCapacityExhausted, 1);
            };
            if self.stream {
                self.input = Some(
                    io.take_input(PortId(0))
                        .expect("present streaming local-model input"),
                );
            } else {
                io.consume(PortId(0)).expect("present local-model input");
            }
            io.request_host_call(request, HostCallId(0), input)
                .expect("local-model Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed local-model input closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn retains_host_call_input(
        &self,
        _request: RequestId,
        value: conduit_kernel::ValueRef,
    ) -> bool {
        self.input == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.input = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl LocalModelOperation {}

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let contract = conduit_ai::llm_contract(placement.kind_id.as_str())
        .ok_or_else(|| "planned local-model Kind is not an L0 semantic contract".to_string())?;
    if !matches!(
        placement.kind_id.as_str(),
        conduit_ai::LLM_GENERATE_KIND
            | conduit_ai::LLM_GENERATE_FLOW_KIND
            | conduit_ai::LLM_CLASSIFY_KIND
            | conduit_ai::LLM_EXTRACT_KIND
            | conduit_ai::LLM_EMBED_KIND
            | conduit_ai::LLM_INTERPRET_KIND
            | conduit_ai::LLM_PRESENT_KIND
            | conduit_ai::LLM_STREAM_GENERATE_KIND
    ) || placement.kind_contract_revision != contract.kind_contract_revision
        || placement.execution_profile_id.as_str() != conduit_ai::LOCAL_MODEL_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        || !placement
            .artifact_id
            .as_str()
            .starts_with(conduit_ai::LOCAL_MODEL_ARTIFACT)
        || placement.inputs != contract.inputs
        || placement.outputs != contract.outputs
        || placement.host_calls.len() != 1
        || placement.host_calls[0].contract_id.as_str() != conduit_ai::LOCAL_MODEL_OPERATION
    {
        return Err("planned local-model identity does not match its installation".to_string());
    }
    for key in [
        "maximum-input-bytes",
        "maximum-context-items",
        "maximum-output-bytes",
        "maximum-work-units",
        "maximum-history-items",
    ] {
        configuration_count(placement, key)?;
    }
    Ok(())
}

pub(super) fn configuration_count(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("local-model configuration '{key}' is missing"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let maximum_input_bytes = u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
        .map_err(|_| "local-model input bound does not fit the kernel".to_string())?;
    let maximum_output_bytes =
        u32::try_from(configuration_count(placement, "maximum-output-bytes")?)
            .map_err(|_| "local-model output bound does not fit the kernel".to_string())?;
    Ok(OperationBudget {
        value_items: if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
            conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNKS as u16
        } else {
            1
        },
        value_bytes: if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
            maximum_output_bytes.saturating_add(
                (conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNKS as u32).saturating_mul(128),
            )
        } else {
            maximum_output_bytes
        },
        host_requests: if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
            conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNKS as usize + 1
        } else {
            1
        },
        sign_items: 32,
        maximum_value_bytes: maximum_input_bytes.max(
            if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
                conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES as u32
            } else {
                maximum_output_bytes
            },
        ),
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::LocalModel(LocalModelOperation {
        maximum_input_bytes: u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
            .map_err(|_| "local-model input bound does not fit the kernel".to_string())?,
        pending: None,
        next_request: 0,
        closed: false,
        flow: placement.kind_id.as_str() == conduit_ai::LLM_GENERATE_FLOW_KIND,
        emitted: false,
        stream: placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND,
        stream_complete: false,
        input: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        HostCallOutcome, ValueRef,
    };

    fn value(slot: u16, bytes: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: bytes,
        }
    }

    #[test]
    fn streaming_operation_pulls_one_chunk_only_after_prior_delivery() {
        let mut operation = LocalModelOperation {
            maximum_input_bytes: 64,
            pending: None,
            next_request: 0,
            closed: false,
            flow: false,
            emitted: false,
            stream: true,
            stream_complete: false,
            input: None,
        };
        let input = value(1, 12);
        let mut io = StepIo::test_frame([Some(input)], [false], [Some(64)], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(0)));
        assert!(io.test_retained(PortId(0)));
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [Some(64)],
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value(2, 20), 64).unwrap()),
                    failure: None,
                },
            )),
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(value(2, 20)));
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(1))
        );
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [Some(64)],
            Some((
                RequestId(1),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_host_completion_consumed());
        assert!(io.test_discards().contains(&Some(input)));
    }
}
