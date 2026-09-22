//! Installed bounded commit from recognition events to one durable user turn.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::RECOGNIZED_TURN_COMMIT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct RecognizedTurnCommitBack {
    pending: Option<RequestId>,
    next_request: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for RecognizedTurnCommitBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
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
                        .expect("observed recognition commit completion");
                    io.send(PortId(0), output.value)
                        .expect("ready committed user turn output");
                    self.pending = None;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed empty recognition commit completion");
                    self.pending = None;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 0),
                (HostCallDisposition::Failed, None, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::InvalidLifecycle, 3),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(FailureCode::InvalidLifecycle, 4);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
            ) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::StorageExhausted, 2);
            };
            io.consume(PortId(0))
                .expect("present recognition event input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("recognition commit Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed recognition event closure");
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

impl RecognizedTurnCommitBack {}

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

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
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
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned recognized-turn commit differs from installed realization".into());
    }
    Ok(())
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::RecognizedTurnCommit(
        RecognizedTurnCommitBack {
            pending: None,
            next_request: 0,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_tongues::{SegmentId, StreamEvent, TextRole};

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
