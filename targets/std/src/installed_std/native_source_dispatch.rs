//! Installed dispatch for the independently selected immediate Source.
use super::*;

pub(super) fn complete(
    source: &mut crate::selected_native_source::PreparedNativeSource<'_>,
    request: conduit_kernel::scheduler::HostCallRequest,
    scheduler: &mut InstalledScheduler,
    host_calls: &mut prepared_host_calls::PreparedHostCalls<'_>,
    maximum_output_bytes: u32,
) -> Result<(), String> {
    let input = scheduler
        .host_value(request.input.value)
        .map_err(|error| format!("read native Source input: {error:?}"))?;
    let Some(encoded) = source.read(request, input)? else {
        return Ok(());
    };
    let value = scheduler
        .store_host_value(encoded)
        .map_err(|error| format!("store selected native Source value: {error:?}"))?;
    host_calls.record(request)?;
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, maximum_output_bytes).map_err(|error| {
                        format!("bound selected native Source value: {error:?}")
                    })?,
                ),
                failure: None,
            },
        )
        .map_err(|error| format!("complete selected native Source: {error:?}"))?;
    Ok(())
}

pub(crate) fn supports_with_native(
    fragment: &PlanFragment,
    source: Option<&crate::selected_native_source::InitializedNativeSource>,
) -> bool {
    !fragment.placements.is_empty()
        && fragment.placements.iter().all(|gear| {
            source.is_some_and(|source| source.matches(gear))
                || catalog::factory(&gear.implementation_id).is_some()
                || gear.implementation_id.as_str()
                    == conduit_std_offers::STATE_VALUE_STD_IMPLEMENTATION
                || gear.implementation_id.as_str()
                    == conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
        })
}

/// Poll the sole outstanding worker request; cancellation retains cleanup custody.
pub(super) fn poll(
    source: &mut crate::selected_native_source::PreparedNativeSource<'_>,
    cancelled: bool,
    scheduler: &mut InstalledScheduler,
    host_calls: &mut prepared_host_calls::PreparedHostCalls<'_>,
) -> Result<(), String> {
    if cancelled {
        source.cancel();
    }
    let Some((request, encoded)) = source.poll()? else {
        return Ok(());
    };
    let maximum = host_calls
        .get(request.node, request.call)
        .ok_or("pending native Source lost its lowered contract")?
        .binding
        .maximum_output_bytes;
    let value = scheduler
        .store_host_value(encoded)
        .map_err(|e| format!("store pending native Source: {e:?}"))?;
    host_calls.record(request)?;
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, maximum)
                        .map_err(|e| format!("bound pending native Source: {e:?}"))?,
                ),
                failure: None,
            },
        )
        .map_err(|e| format!("complete pending native Source: {e:?}"))?;
    Ok(())
}
