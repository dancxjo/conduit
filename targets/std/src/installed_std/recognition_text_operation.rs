//! Installed projection from a canonical recognition result to bounded text.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECOGNITION_TO_TEXT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecognitionTextOperation {
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
    flow: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for RecognitionTextOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted && !self.flow {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed recognition-to-text completion");
                    io.send(PortId(0), output.value)
                        .expect("ready recognized text output");
                    self.pending = None;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, _, _) => step_fail(FailureCode::HostCallDenied, 2),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::HostCallFailed, 3),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || (!self.flow && self.emitted) {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            let maximum = if self.flow {
                conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32
            } else {
                conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32
            };
            let Ok(input) = BoundedValueRef::new(value, maximum) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::IdentityCapacityExhausted, 1);
            };
            io.consume(PortId(0))
                .expect("present recognition result input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("recognition-to-text Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if self.flow && io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed recognition stream closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl RecognitionTextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && (self.flow || !self.emitted) => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    if self.flow {
                        conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32
                    } else {
                        conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32
                    },
                ) else {
                    return fail(FailureCode::InvalidInput, 1);
                };
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.saturating_add(1);
                self.pending = Some(request);
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Denied, _, _) => fail(FailureCode::HostCallDenied, 2),
                    _ => fail(FailureCode::HostCallFailed, 3),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() && self.flow => {
                OperationAction::Complete
            }
            _ => fail(FailureCode::InvalidLifecycle, 4),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted && !self.flow {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = if placement.kind_id.as_str() == conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND {
        conduit_std_offers::committed_turn_to_text_std_offer()
    } else {
        conduit_std_offers::recognition_to_text_std_offer()
    };
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::RECOGNITION_TO_TEXT_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::RECOGNITION_TO_TEXT_STD_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::RECOGNITION_TO_TEXT_STD_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned recognition-to-text identity does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: if placement.kind_id.as_str()
            == conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND
        {
            conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32
        } else {
            conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES
        },
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecognitionText(
        RecognitionTextOperation {
            pending: None,
            next_request: 0,
            emitted: false,
            flow: placement.kind_id.as_str() == conduit_tongues::COMMITTED_TURN_TO_TEXT_KIND,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
