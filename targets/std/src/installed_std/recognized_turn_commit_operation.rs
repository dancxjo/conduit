//! Installed bounded commit from recognition events to one durable user turn.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::RECOGNIZED_TURN_COMMIT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecognizedTurnCommitOperation {
    pending: Option<RequestId>,
    next_request: u32,
}

impl RecognizedTurnCommitOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
                ) else {
                    return fail(1);
                };
                let request = RequestId(self.next_request);
                let Some(next) = self.next_request.checked_add(1) else {
                    return fail(2);
                };
                self.next_request = next;
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
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(3),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(4),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) struct RecognizedTurnCommitHost {
    output: Vec<u8>,
}

impl RecognizedTurnCommitHost {
    fn new() -> Self {
        Self {
            output: Vec::with_capacity(conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES),
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Option<&[u8]>, String> {
        let event = conduit_tongues::decode_recognition_event(input)
            .map_err(|error| format!("decode Tongues recognition event: {error:?}"))?;
        let message = conduit_tongues::committed_user_message(&event)
            .map_err(|error| format!("project Tongues recognition commit: {error:?}"))?;
        self.output.clear();
        if let Some(message) = message {
            self.output = conduit_tongues::encode_committed_user_message(&message)
                .map_err(|error| format!("encode committed Body turn: {error:?}"))?;
            Ok(Some(&self.output))
        } else {
            Ok(None)
        }
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<RecognizedTurnCommitHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::RECOGNIZED_TURN_COMMIT_IMPLEMENTATION)
                .then(RecognizedTurnCommitHost::new)
        })
        .collect()
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES
            + conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES) as u32,
        host_requests: usize::from(conduit_tongues::MAXIMUM_RECOGNITION_EVENT_ITEMS),
        sign_items: 32,
        maximum_value_bytes: conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES as u32,
    })
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::recognized_turn_commit_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
    {
        return Err("planned recognized-turn commit differs from installed realization".into());
    }
    Ok(())
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::RecognizedTurnCommit(
        RecognizedTurnCommitOperation {
            pending: None,
            next_request: 0,
        },
    ))
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaking::{SegmentId, StreamEvent, TextRole};

    fn encoded(event: StreamEvent) -> Vec<u8> {
        conduit_tongues::encode_recognition_event(&event).expect("encode Tongues event")
    }

    #[test]
    fn only_tongues_committed_recognition_emits_a_body_turn() {
        let mut host = RecognizedTurnCommitHost::new();
        let partial = StreamEvent::PartialHypothesis {
            role: TextRole::Recognition,
            segment_id: SegmentId("turn-1".into()),
            text: "Hello".into(),
            confidence: None,
        };
        assert!(host.execute(&encoded(partial)).unwrap().is_none());

        let committed = StreamEvent::CommittedSegment {
            role: TextRole::Recognition,
            segment_id: SegmentId("turn-1".into()),
            text: "Hello there".into(),
            words: Vec::new(),
            language: None,
            speaker_id: None,
            confidence: None,
        };
        let output = host
            .execute(&encoded(committed))
            .unwrap()
            .expect("committed Body message");
        let message: conduit_tongues::CommittedUserMessage =
            serde_json::from_slice(output).unwrap();
        assert_eq!(message.text, "Hello there");
    }
}
