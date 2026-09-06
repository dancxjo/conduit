//! Synchronous value transformations for admitted browser Host requests.
//! This dispatcher never advances the scheduler or owns pending platform effects.
use super::{debug_error, TourScheduler};
use conduit_core::{HostOperationRequirement, PlannedGear};
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) fn complete_transform(
    scheduler: &mut TourScheduler,
    placement: &PlannedGear,
    operation: &HostOperationRequirement,
    request: HostOperationRequest,
) -> Result<bool, String> {
    if crate::installed_browser::timing::OPERATIONS.contains(&operation.contract_id.as_str()) {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.timing[usize::from(request.node.0)]
            .as_mut()
            .ok_or("timing codec was not prepared before Play")?
            .execute(operation.contract_id.as_str(), input);
        let outcome = match result {
            Ok(bytes) => HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_operation(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::json::OPERATIONS.contains(&operation.contract_id.as_str()) {
        let result = crate::installed_browser::json::execute(
            placement,
            operation.contract_id.as_str(),
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
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
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::pointer_selector::HOST_OPERATION
    {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let output = scheduler.selectors[usize::from(request.node.0)]
            .as_mut()
            .ok_or("selector was not prepared before Play")?
            .execute(input)?;
        let value = scheduler
            .kernel
            .store_host_value(output)
            .map_err(debug_error)?;
        scheduler
            .kernel
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::QUANTITY_HOST_OPERATION {
        let encoded = crate::installed_browser::transform_quantity(
            scheduler.mappings[usize::from(request.node.0)]
                .ok_or("quantity mapping was not prepared before Play")?,
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        )?;
        let outcome = match encoded {
            Ok(bytes) => {
                let value = scheduler.store_host_value(&bytes).map_err(debug_error)?;
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                }
            }
            Err(failure) => HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_operation(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::NORMALIZE_QUANTITY_OPERATION {
        let converted = crate::installed_browser::normalize_quantity(
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match converted {
            Ok(bytes) => HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        conduit_core::SCALAR_ENCODED_LEN as u32,
                    )
                    .map_err(debug_error)?,
                ),
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
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::QUANTITY_WRAP_OPERATION {
        let (encoded, length) = crate::installed_browser::wrap_quantity(
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        )?;
        let value = scheduler
            .store_host_value(&encoded[..length])
            .map_err(debug_error)?;
        scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        return Ok(true);
    }
    Ok(false)
}
