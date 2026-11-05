//! Complete the same prepared scalar Backs under the shared Body scheduler.
//! No new provider, Plan configuration copy, or execution-time transform parsing.
use super::*;
use conduit_core::SCALAR_ENCODED_LEN;

pub(super) fn supports(operation: &LoweredHostCall) -> bool {
    let pair = operation.target_kind.as_ref().is_some_and(|kind| {
        matches!(
            (operation.contract_id.as_str(), kind.as_str()),
            (
                conduit_std_offers::MATH_CLAMP_HOST_CALL,
                conduit_semantic_catalog::MATH_CLAMP_KIND
            ) | (
                conduit_std_offers::MATH_SCALE_HOST_CALL,
                conduit_semantic_catalog::MATH_SCALE_KIND
            ) | (
                conduit_std_offers::MATH_DEADBAND_HOST_CALL,
                conduit_semantic_catalog::MATH_DEADBAND_KIND
            )
        )
    });
    pair && operation.binding.maximum_input_bytes == SCALAR_ENCODED_LEN as u32
        && operation.binding.maximum_output_bytes == SCALAR_ENCODED_LEN as u32
}

pub(super) fn complete(
    operation: &LoweredHostCall,
    request: HostCallRequest,
    scheduler: &mut InstalledScheduler,
) -> Result<bool, String> {
    if !supports(operation) {
        return Ok(false);
    }
    if operation.node != request.node
        || operation.call != request.call
        || request.input.admitted_bytes != SCALAR_ENCODED_LEN as u32
    {
        return Err("Body scalar call differs from its exact selected operation".into());
    }
    let back = scheduler
        .drivers()
        .get(usize::from(request.node.0))
        .ok_or("Body scalar call has no prepared Back")?;
    let input = scheduler
        .host_value(request.input.value)
        .map_err(|error| format!("Body scalar input: {error:?}"))?;
    let encoded = super::super::math_backs::transform_bytes(
        back,
        operation
            .target_kind
            .as_ref()
            .ok_or("Body scalar target absent")?,
        input,
    )?;
    let value = scheduler
        .store_host_value(&encoded)
        .map_err(|error| format!("Body scalar output: {error:?}"))?;
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, SCALAR_ENCODED_LEN as u32)
                        .map_err(|error| format!("Body scalar output bound: {error:?}"))?,
                ),
                failure: None,
            },
        )
        .map_err(|error| format!("Body scalar completion: {error:?}"))?;
    Ok(true)
}
