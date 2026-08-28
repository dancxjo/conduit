//! Ordinary installed operation for the portable vector-search Host boundary.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static EXACT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_ai::EXACT_VECTOR_SEARCH_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static HNSW_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: crate::hosted_vector_index::HOSTED_HNSW_IMPLEMENTATION_ID,
    budget,
    prepare,
};

pub(super) struct VectorSearchOperation {
    maximum_input_bytes: u32,
    pending: bool,
    emitted: bool,
}

impl VectorSearchOperation {
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
                    return fail(FailureCode::InvalidInput, 61);
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
                        fail(FailureCode::HostOperationDenied, 62)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 63),
                    (HostOperationDisposition::Failed, _, _) => {
                        fail(FailureCode::HostOperationFailed, 64)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 65),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 66),
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

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let contract = conduit_ai::vector_search_contract();
    if placement.kind_id != contract.kind_id
        || placement.kind_contract_revision != contract.kind_contract_revision
        || !matches!(
            placement.implementation_id.as_str(),
            conduit_ai::EXACT_VECTOR_SEARCH_IMPLEMENTATION
                | crate::hosted_vector_index::HOSTED_HNSW_IMPLEMENTATION_ID
        )
        || placement.inputs != contract.inputs
        || placement.outputs != contract.outputs
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != conduit_ai::VECTOR_SEARCH_OPERATION
    {
        return Err("planned vector-search identity does not match its installation".into());
    }
    for key in [
        "maximum-input-bytes",
        "maximum-output-bytes",
        "maximum-query-work-units",
        "maximum-results",
    ] {
        configuration_count(placement, key)?;
    }
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
        .ok_or_else(|| format!("vector-search configuration '{key}' is missing"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let maximum_input_bytes = u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
        .map_err(|_| "vector-search input bound does not fit the kernel".to_string())?;
    let maximum_output_bytes =
        u32::try_from(configuration_count(placement, "maximum-output-bytes")?)
            .map_err(|_| "vector-search output bound does not fit the kernel".to_string())?;
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
    Ok(InstalledOperation::VectorSearch(VectorSearchOperation {
        maximum_input_bytes: u32::try_from(configuration_count(placement, "maximum-input-bytes")?)
            .map_err(|_| "vector-search input bound does not fit the kernel".to_string())?,
        pending: false,
        emitted: false,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
