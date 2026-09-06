//! Storage effects beneath the existing browser scheduler. No scheduling policy.
use super::*;
use crate::resource_snapshot::{
    PreparedSnapshotRecord, SnapshotRefusal, PUBLISH_OPERATION, READ_OPERATION,
};
use conduit_core::{AuthorityBinding, PlannedGear};
use conduit_kernel::{Failure, FailureCode};

pub(super) struct SnapshotState {
    record: PreparedSnapshotRecord,
    reference: Vec<u8>,
    authority: AuthorityBinding,
    pending: Option<conduit_kernel::RequestId>,
}
impl SnapshotState {
    pub(super) fn prepare(placement: &PlannedGear) -> Result<Self, String> {
        let reference = crate::installed_browser::resource::reference(placement)?;
        let record = PreparedSnapshotRecord::prepare(placement, &reference).map_err(debug_error)?;
        Ok(Self {
            record,
            reference: reference.encode().map_err(debug_error)?,
            authority: placement.authority[0].clone(),
            pending: None,
        })
    }
}
pub(super) fn matches(contract: &str) -> bool {
    matches!(contract, PUBLISH_OPERATION | READ_OPERATION)
}

pub(super) fn begin(
    scheduler: &mut TourScheduler,
    placement: &PlannedGear,
    request: HostOperationRequest,
) -> Result<Option<PendingHostEffect>, String> {
    let publish = placement.host_operations[0].contract_id.as_str() == PUBLISH_OPERATION;
    let input = scheduler
        .kernel
        .host_value(request.input.value)
        .map_err(debug_error)?;
    let state = scheduler.snapshots[usize::from(request.node.0)]
        .as_mut()
        .ok_or("snapshot was not prepared")?;
    let failure = if publish {
        match conduit_web::JsonValue::decode_text(input) {
            Ok(_) => state
                .record
                .publication(&state.authority, input)
                .err()
                .map(snapshot_failure),
            Err(error) => Some(Failure {
                code: FailureCode::InvalidInput,
                detail: error as u16,
            }),
        }
    } else if input != state.reference {
        Some(Failure {
            code: FailureCode::InvalidInput,
            detail: 208,
        })
    } else {
        None
    };
    if let Some(failure) = failure {
        scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure: Some(failure),
                },
            )
            .map_err(debug_error)?;
        return Ok(None);
    }
    state.pending = Some(request.request);
    Ok(Some(PendingHostEffect {
        request,
        effect: BrowserHostEffect::Snapshot { publish },
    }))
}

#[derive(serde::Serialize)]
pub(in crate::form_runner) struct StorageRequest<'a> {
    pub effect_kind: &'static str,
    pub key: &'a str,
    pub record: Option<&'a [u8]>,
}

pub(in crate::form_runner) fn describe<'a>(
    scheduler: &'a TourScheduler,
    pending: &PendingHostEffect,
) -> Result<StorageRequest<'a>, String> {
    let BrowserHostEffect::Snapshot { publish } = pending.effect else {
        return Err("not a snapshot request".into());
    };
    let state = scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_ref()
        .ok_or("snapshot was not prepared")?;
    Ok(StorageRequest {
        effect_kind: if publish {
            "resource-publish"
        } else {
            "resource-read"
        },
        key: state.record.storage_key(),
        record: if publish {
            state.record.candidate_record()
        } else {
            None
        },
    })
}

/// Acknowledgement completes the original kernel request. Failed storage cannot
/// emit a successful ResourceRef; read bytes must validate before entering a Cord.
pub(super) fn complete(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
    result: Result<Option<&[u8]>, Failure>,
) -> Result<(), String> {
    let BrowserHostEffect::Snapshot { publish } = pending.effect else {
        return Err("not a snapshot request".into());
    };
    let state = scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_ref()
        .ok_or("snapshot was not prepared")?;
    if state.pending != Some(pending.request.request) {
        return Err("snapshot completion is stale".into());
    }
    let result = result.and_then(|record| match (publish, record) {
        (true, None) => Ok(state.reference.as_slice()),
        (false, Some(record)) => state
            .record
            .restore(&state.authority, record)
            .map_err(snapshot_failure),
        _ => Err(Failure {
            code: FailureCode::HostOperationFailed,
            detail: 209,
        }),
    });
    let outcome = match result {
        Ok(bytes) => HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: Some(
                BoundedValueRef::new(
                    scheduler
                        .kernel
                        .store_host_value(bytes)
                        .map_err(debug_error)?,
                    if publish { 512 } else { 4096 },
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
    let output = outcome.output.map(|value| value.value);
    let completion =
        scheduler.complete_host_operation(pending.request.node, pending.request.request, outcome);
    scheduler.snapshots[usize::from(pending.request.node.0)]
        .as_mut()
        .expect("prepared snapshot")
        .pending = None;
    if completion.is_err() {
        if let Some(output) = output {
            scheduler.discard_host_value(output).map_err(debug_error)?;
        }
    }
    completion.map_err(debug_error)
}

fn snapshot_failure(error: SnapshotRefusal) -> Failure {
    let detail = match error {
        SnapshotRefusal::InvalidBinding => 200,
        SnapshotRefusal::ForeignHost => 201,
        SnapshotRefusal::StaleBoot => 202,
        SnapshotRefusal::UnsupportedLifetime => 203,
        SnapshotRefusal::AuthorityDenied => 204,
        SnapshotRefusal::Reference(_) => 205,
        SnapshotRefusal::WrongAccess => 206,
        SnapshotRefusal::ContentExtent => 207,
        SnapshotRefusal::CorruptRecord => 208,
        SnapshotRefusal::UnsupportedExpiry => 210,
    };
    Failure {
        code: FailureCode::HostOperationFailed,
        detail,
    }
}
