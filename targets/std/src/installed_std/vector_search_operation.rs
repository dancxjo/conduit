//! Ordinary installed operation for the portable vector-search Host boundary.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
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

impl<const PORTS: usize> StepOperation<PORTS> for VectorSearchOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return step_fail(FailureCode::InvalidLifecycle, 66);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed vector-search completion");
                    io.send(PortId(0), output.value)
                        .expect("ready vector-search output");
                    self.pending = false;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 62),
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 63),
                (HostCallDisposition::Failed, _, _) => {
                    StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                        code: FailureCode::HostCallFailed,
                        detail: 64,
                    }))
                }
                _ => step_fail(FailureCode::InvalidLifecycle, 65),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return step_fail(FailureCode::InvalidLifecycle, 66);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return step_fail(FailureCode::InvalidInput, 61);
            };
            io.consume(PortId(0)).expect("present vector-search query");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("vector-search Host Call");
            self.pending = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl VectorSearchOperation {}

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
        || placement.host_calls.len() != 1
        || placement.host_calls[0].contract_id.as_str() != conduit_ai::VECTOR_SEARCH_OPERATION
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
