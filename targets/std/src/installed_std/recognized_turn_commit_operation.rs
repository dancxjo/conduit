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
    committer: Option<conduit_tongues::RecognizedTurnCommitter>,
    output: Vec<u8>,
}

impl RecognizedTurnCommitHost {
    fn new() -> Self {
        Self {
            committer: None,
            output: Vec::with_capacity(conduit_tongues::MAXIMUM_COMMITTED_USER_MESSAGE_BYTES),
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Option<&[u8]>, String> {
        let event: conduit_tongues::RecognitionEvent = serde_json::from_slice(input)
            .map_err(|error| format!("decode recognition event: {error}"))?;
        if self.committer.is_none() {
            self.committer = Some(conduit_tongues::RecognizedTurnCommitter::new(
                event.stream_id.clone(),
            ));
        }
        let (_, message, _) = self
            .committer
            .as_mut()
            .expect("committer initialized")
            .accept(&event)
            .map_err(|error| format!("commit recognized turn: {error:?}"))?;
        self.output.clear();
        if let Some(message) = message {
            self.output = serde_json::to_vec(&message)
                .map_err(|error| format!("encode committed turn: {error}"))?;
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
    use conduit_tongues::{RecognitionEvent, RecognitionEventStatus, SpeechOrigin};

    fn event(sequence: u32, status: RecognitionEventStatus, text: Option<&str>) -> Vec<u8> {
        serde_json::to_vec(&RecognitionEvent {
            stream_id: "recognition/test-turn".into(),
            sequence,
            status,
            origin: SpeechOrigin::External,
            text: text.map(str::to_string),
            audio_extent_bytes: 320,
            elapsed_milliseconds: 20,
            provider_identity: "test/recognizer".into(),
        })
        .expect("encode event")
    }

    #[test]
    fn provisional_waits_and_committed_event_emits_exactly_once() {
        let mut host = RecognizedTurnCommitHost::new();
        assert!(host
            .execute(&event(
                0,
                RecognitionEventStatus::Provisional,
                Some("Hello")
            ))
            .expect("provisional")
            .is_none());
        let encoded = host
            .execute(&event(
                1,
                RecognitionEventStatus::Committed,
                Some("Hello there"),
            ))
            .expect("committed")
            .expect("message");
        let message: conduit_tongues::CommittedUserMessage =
            serde_json::from_slice(encoded).expect("decode message");
        assert_eq!(message.text, "Hello there");
        assert!(host
            .execute(&event(
                1,
                RecognitionEventStatus::Committed,
                Some("Hello there")
            ))
            .is_err());
    }
}
