//! Portable microphone clip capture realized through one admitted host operation.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct MicrophoneClipOperation {
    pending: bool,
    emitted: bool,
}

impl MicrophoneClipOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, 16) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 3),
                    _ => fail(FailureCode::HostOperationFailed, 4),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 5),
        }
    }

    pub(super) fn advance(&self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

fn offer() -> CapabilityOffer {
    conduit_std_offers::microphone_clip_offer()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.resources.len() != 1
        || placement.authority.len() != 1
        || !placement.configuration.is_empty()
    {
        return Err("planned microphone clip capture does not match std installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::MicrophoneClip(
        MicrophoneClipOperation {
            pending: false,
            emitted: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
