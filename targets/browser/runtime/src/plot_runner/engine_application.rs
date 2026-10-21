//! Retained application Host Call completion over the prepared source instance.
//! This module neither advances the scheduler nor owns another application model.
use super::{debug_error, TourScheduler};
use conduit_core::HostCallRequirement;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

pub(super) fn complete_call(
    scheduler: &mut TourScheduler,
    operation: &HostCallRequirement,
    request: HostCallRequest,
) -> Result<bool, String> {
    if operation.contract_id.as_str() == crate::installed_browser::application::STATE_OPERATION {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let index = usize::from(request.node.0);
        let result = scheduler.applications[index]
            .as_mut()
            .ok_or("browser application state was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(&bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(_) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    detail: 70,
                }),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    Ok(false)
}
