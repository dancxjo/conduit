//! Installed bounded relative-duration normalization.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::NORMALIZE_SEQUENCE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct SequenceNormalizationOperation {
    pending: bool,
    completed: bool,
}

impl SequenceNormalizationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.completed => {
                self.pending = true;
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(240);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending => {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.completed = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => InstalledOperation::fail(241),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.completed && !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(242),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.completed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(super) use conduit_semantic_catalog::BoundedNormalizationCodec as SequenceNormalizationHost;

pub(super) fn refusal_detail(
    refusal: &conduit_semantic_catalog::SequenceNormalizationRefusal,
) -> u16 {
    use conduit_semantic_catalog::SequenceNormalizationRefusal::*;
    match refusal {
        Malformed => 1,
        Empty => 2,
        TooManyValues => 3,
        ZeroDuration => 4,
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = conduit_std_offers::normalize_sequence_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
    {
        return Err("planned sequence normalization differs from installed realization".into());
    }
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::SequenceNormalization(
        SequenceNormalizationOperation {
            pending: false,
            completed: false,
        },
    ))
}
