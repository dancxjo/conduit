use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static GENERATE_TEXT_SMALL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_ai::SMALL_LOCAL_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static GENERATE_TEXT_LARGE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_ai::LARGE_LOCAL_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static GENERATE_TEXT_REMOTE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_ai::REMOTE_FRONTIER_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct GenerateTextOperation {
    maximum_input_bytes: u32,
    pending: bool,
    emitted: bool,
}

impl GenerateTextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
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
            _ => fail(FailureCode::InvalidLifecycle, 6),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(super) fn execute_fixture(
    placement: &PlannedGear,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Result<(), String> {
    validate(placement)?;
    let prompt = core::str::from_utf8(input)
        .map_err(|_| "generate-text fixture input is not valid UTF-8".to_string())?;
    output.clear();
    let label = match placement.implementation_id.as_str() {
        conduit_ai::SMALL_LOCAL_IMPLEMENTATION => "small-local",
        conduit_ai::LARGE_LOCAL_IMPLEMENTATION => "large-local",
        conduit_ai::REMOTE_FRONTIER_IMPLEMENTATION => "remote-frontier",
        _ => return Err("planned generate-text implementation is not installed".to_string()),
    };
    use std::io::Write;
    write!(output, "fixture/{label}: {prompt}").map_err(|error| error.to_string())?;
    let maximum = configuration_count(placement, "maximum-output-tokens")?
        .checked_mul(4)
        .ok_or_else(|| "generate-text output byte bound overflow".to_string())?;
    if output.len() as u64 > maximum {
        return Err("generate-text fixture output exceeds the planned bound".to_string());
    }
    Ok(())
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let artifact_matches = matches!(
        (
            placement.implementation_id.as_str(),
            placement.artifact_id.as_str()
        ),
        (
            conduit_ai::SMALL_LOCAL_IMPLEMENTATION,
            conduit_ai::SMALL_LOCAL_ARTIFACT
        ) | (
            conduit_ai::LARGE_LOCAL_IMPLEMENTATION,
            conduit_ai::LARGE_LOCAL_ARTIFACT
        ) | (
            conduit_ai::REMOTE_FRONTIER_IMPLEMENTATION,
            conduit_ai::REMOTE_FRONTIER_ARTIFACT
        )
    );
    if placement.kind_id.as_str() != conduit_ai::GENERATE_TEXT_KIND
        || placement.kind_contract_revision.as_str() != conduit_ai::GENERATE_TEXT_REVISION
        || placement.execution_profile_id.as_str() != "conduit.ai/generate-text-hosted@1"
        || !artifact_matches
        || placement.inputs.len() != 1
        || placement.outputs.len() != 1
        || placement.inputs[0].port_id.as_str() != "prompt"
        || placement.inputs[0].value_kind.as_str() != conduit_ai::TEXT_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
        || placement.outputs[0].port_id.as_str() != "text"
        || placement.outputs[0].value_kind.as_str() != conduit_ai::TEXT_VALUE_KIND
        || placement.outputs[0].direction != PortDirection::Output
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str()
            != conduit_ai::GENERATE_TEXT_HOST_OPERATION
    {
        return Err("planned generate-text identity does not match its installation".to_string());
    }
    configuration_count(placement, "maximum-input-bytes")?;
    configuration_count(placement, "maximum-output-tokens")?;
    Ok(())
}

fn configuration_count(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("generate-text configuration '{key}' is missing"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let maximum_input_bytes = u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
        .map_err(|_| "generate-text input bound does not fit the kernel".to_string())?;
    let maximum_output_bytes = configuration_count(placement, "maximum-output-tokens")?
        .checked_mul(4)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "generate-text output bound does not fit the kernel".to_string())?;
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
    Ok(InstalledOperation::GenerateText(GenerateTextOperation {
        maximum_input_bytes: u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
            .map_err(|_| "generate-text input bound does not fit the kernel".to_string())?,
        pending: false,
        emitted: false,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
