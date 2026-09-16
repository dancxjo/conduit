use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
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
}

impl LocalModelOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.closed && (self.flow || !self.emitted) => {
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 3),
                    (HostOperationDisposition::Failed, _, _) => {
                        fail(FailureCode::HostOperationFailed, 4)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 5),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.closed = true;
                OperationAction::Complete
            }
            _ => fail(FailureCode::InvalidLifecycle, 6),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted && !self.flow {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

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
    ) || placement.kind_contract_revision != contract.kind_contract_revision
        || placement.execution_profile_id.as_str() != conduit_ai::LOCAL_MODEL_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        || !placement
            .artifact_id
            .as_str()
            .starts_with(conduit_ai::LOCAL_MODEL_ARTIFACT)
        || placement.inputs != contract.inputs
        || placement.outputs != contract.outputs
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != conduit_ai::LOCAL_MODEL_OPERATION
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
        value_items: 1,
        value_bytes: maximum_output_bytes,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: maximum_input_bytes.max(maximum_output_bytes),
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
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
