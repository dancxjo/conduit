//! Prepared measurement-window Host Call dispatch.

use super::{debug_error, TourScheduler};
use conduit_core::HostCallRequirement;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

pub(super) fn complete(
    scheduler: &mut TourScheduler,
    operation: &HostCallRequirement,
    request: HostCallRequest,
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
        Ok(output) => HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: output
                .map(|bytes| {
                    let value = scheduler.store_host_value(&bytes).map_err(debug_error)?;
                    BoundedValueRef::new(value, operation.maximum_output_bytes).map_err(debug_error)
                })
                .transpose()?,
            failure: None,
        },
        Err(failure) => HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    scheduler
        .complete_host_call(request.node, request.request, outcome)
        .map_err(debug_error)
}
