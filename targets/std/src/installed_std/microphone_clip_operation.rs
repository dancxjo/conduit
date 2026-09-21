//! Portable microphone clip capture realized through one admitted Host Call.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId,
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

impl<const PORTS: usize> StepOperation<PORTS> for MicrophoneClipOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return step_fail(FailureCode::InvalidLifecycle, 5);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed microphone capture completion");
                    io.send(PortId(0), output.value)
                        .expect("ready microphone clip output");
                    self.pending = false;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 2),
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 3),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::HostCallFailed, 4),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return step_fail(FailureCode::InvalidLifecycle, 5);
            }
            let Ok(input) = BoundedValueRef::new(value, 16) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            io.consume(PortId(0))
                .expect("present microphone capture request");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("microphone capture Host Call");
            self.pending = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
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
                OperationAction::RequestHostCall {
                    request: RequestId(0),
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending && request == RequestId(0) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 2),
                    (HostCallDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 3),
                    _ => fail(FailureCode::HostCallFailed, 4),
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
        || placement.host_calls != offer.host_calls
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
