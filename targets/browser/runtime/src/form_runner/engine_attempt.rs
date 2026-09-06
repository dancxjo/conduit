//! Clock observations complete the shared codec through the existing kernel.
use super::*;
use conduit_semantic_catalog::{ButtonAttemptObservation, ButtonAttemptRefusal};

pub(super) fn complete_clock(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
    output: &[u8],
) -> Result<(), String> {
    let now = u64::from_le_bytes(
        output
            .try_into()
            .map_err(|_| "clock observation must contain eight bytes")?,
    );
    let codec = scheduler
        .attempts
        .get_mut(usize::from(pending.request.node.0))
        .and_then(Option::as_mut)
        .ok_or("timed attempt codec is absent")?;
    let input = scheduler
        .kernel
        .host_value(pending.request.input.value)
        .map_err(debug_error)?;
    let observation = codec.observe(input, now);
    let (bytes, failure) = match observation {
        Ok(ButtonAttemptObservation::Released) => (None, None),
        Ok(ButtonAttemptObservation::Pressed) => (Some(&[0][..]), None),
        Ok(ButtonAttemptObservation::Complete(bytes)) => (Some(bytes), None),
        Err(refusal) => (
            None,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: match refusal {
                    ButtonAttemptRefusal::MalformedTransition => 1,
                    ButtonAttemptRefusal::TooManyEvents => 2,
                    ButtonAttemptRefusal::ClockRegressed => 3,
                },
            }),
        ),
    };
    let value = bytes
        .map(|bytes| {
            scheduler
                .kernel
                .store_host_value(bytes)
                .map_err(debug_error)
        })
        .transpose()?;
    let result = scheduler
        .kernel
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: if failure.is_some() {
                    HostOperationDisposition::Failed
                } else {
                    HostOperationDisposition::Completed
                },
                output: value
                    .map(|value| BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32))
                    .transpose()
                    .map_err(|_| "timed output exceeds browser bound")?,
                failure,
            },
        )
        .map_err(debug_error);
    if result.is_err() {
        if let Some(value) = value {
            scheduler
                .kernel
                .discard_host_value(value)
                .map_err(debug_error)?;
        }
    }
    result
}
