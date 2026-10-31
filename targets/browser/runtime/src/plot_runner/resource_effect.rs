//! Storage effects beneath the existing browser scheduler. No scheduling policy.
use super::*;
pub(super) use crate::resource_snapshot::PreparedSnapshotOperation as SnapshotState;
use crate::resource_snapshot::{SnapshotStorageRequest, PUBLISH_OPERATION, READ_OPERATION};
use conduit_kernel::Failure;

pub(super) fn matches(contract: &str) -> bool {
    matches!(contract, PUBLISH_OPERATION | READ_OPERATION)
}

pub(super) fn begin(
    scheduler: &mut TourScheduler,
    request: HostCallRequest,
) -> Result<Option<PendingHostEffect>, String> {
    let input = scheduler
        .kernel
        .host_value(request.input.value)
        .map_err(debug_error)?;
    let state = scheduler.snapshots[usize::from(request.node.0)]
        .as_mut()
        .ok_or("snapshot was not prepared")?;
    let publish = state.is_publish();
    if let Err(failure) = state.begin(request, input) {
        scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(failure),
                },
            )
            .map_err(debug_error)?;
        return Ok(None);
    }
    Ok(Some(PendingHostEffect {
        request,
        effect: BrowserHostEffect::Snapshot { publish },
    }))
}

pub(in crate::plot_runner) fn describe<'a>(
    scheduler: &'a TourScheduler,
    pending: &PendingHostEffect,
) -> Result<SnapshotStorageRequest<'a>, String> {
    let BrowserHostEffect::Snapshot { .. } = pending.effect else {
        return Err("not a snapshot request".into());
    };
    let state = scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_ref()
        .ok_or("snapshot was not prepared")?;
    state.describe(pending.request).map_err(debug_error)
}

/// Acknowledgement completes the original kernel request. Failed storage cannot
/// emit a successful ResourceRef; read bytes must validate before entering a Cord.
pub(super) fn complete(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
    result: Result<Option<&[u8]>, Failure>,
) -> Result<(), String> {
    let BrowserHostEffect::Snapshot { .. } = pending.effect else {
        return Err("not a snapshot request".into());
    };
    let state = scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_ref()
        .ok_or("snapshot was not prepared")?;
    state.describe(pending.request).map_err(debug_error)?;
    let bound = state.output_bound();
    let result = state.completion(pending.request, result);
    let outcome = match result {
        Ok(bytes) => HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(
                BoundedValueRef::new(
                    scheduler
                        .kernel
                        .store_host_value(bytes)
                        .map_err(debug_error)?,
                    bound,
                )
                .map_err(debug_error)?,
            ),
            failure: None,
        },
        Err(failure) => HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(failure),
        },
    };
    let output = outcome.output.map(|value| value.value);
    let completion =
        scheduler.complete_host_call(pending.request.node, pending.request.request, outcome);
    scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_mut()
        .expect("prepared snapshot")
        .finish(pending.request)
        .map_err(debug_error)?;
    if completion.is_err() {
        if let Some(output) = output {
            scheduler.discard_host_value(output).map_err(debug_error)?;
        }
    }
    completion.map_err(debug_error)
}
