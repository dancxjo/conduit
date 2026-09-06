//! Installed bounded ordered-event interval derivation.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::ORDERED_EVENT_INTERVALS_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TimedPatternOperation {
    pending: Option<RequestId>,
    completed: bool,
}

impl TimedPatternOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.completed => {
                self.pending = Some(RequestId(0));
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(230);
                };
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
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
                    _ => InstalledOperation::fail(231),
                }
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && self.completed =>
            {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(232),
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
        self.pending = None;
    }
}

pub(super) use conduit_semantic_catalog::BoundedIntervalCodec as TimedPatternHost;

pub(super) fn refusal_detail(refusal: &conduit_semantic_catalog::TimedPatternRefusal) -> u16 {
    use conduit_semantic_catalog::TimedPatternRefusal::*;
    match refusal {
        Malformed => 1,
        TooFewEvents => 2,
        TooManyEvents => 3,
        ReorderedOrDuplicateEvent => 4,
        IntervalOverflow => 5,
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let offer = conduit_std_offers::ordered_event_intervals_std_offer();
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
        return Err("planned ordered-event intervals differ from installed realization".into());
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
    Ok(InstalledOperation::TimedPattern(TimedPatternOperation {
        pending: None,
        completed: false,
    }))
}
