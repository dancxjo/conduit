//! Prepared measurement-window host-operation dispatch.

use super::{debug_error, TourScheduler};
use conduit_core::HostOperationRequirement;
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) fn complete(
    scheduler: &mut TourScheduler,
    operation: &HostOperationRequirement,
    request: HostOperationRequest,
) -> Result<(), String> {
    let input = scheduler
        .kernel
        .host_value(request.input.value)
        .map_err(debug_error)?;
    let result = scheduler.measurement_windows[usize::from(request.node.0)]
        .as_mut()
        .ok_or("measurement window was not prepared before Play")?
        .execute(operation.contract_id.as_str(), input);
    let outcome = match result {
        Ok(output) => HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: output
                .map(|bytes| {
                    let value = scheduler.store_host_value(&bytes).map_err(debug_error)?;
                    BoundedValueRef::new(value, operation.maximum_output_bytes).map_err(debug_error)
                })
                .transpose()?,
            failure: None,
        },
        Err(failure) => HostOperationOutcome {
            disposition: HostOperationDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    scheduler
        .complete_host_operation(request.node, request.request, outcome)
        .map_err(debug_error)
}
