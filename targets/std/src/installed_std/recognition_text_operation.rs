//! Installed projection from a canonical recognition result to bounded text.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
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
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
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
                    _ => fail(FailureCode::HostOperationFailed, 3),
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
        || placement.host_operations != offer.host_operations
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
