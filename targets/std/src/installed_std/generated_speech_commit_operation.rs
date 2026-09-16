//! Installed bounded language-aware commit of generated deltas to speech segments.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};
use std::collections::VecDeque;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::GENERATED_SPEECH_COMMIT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct GeneratedSpeechCommitOperation {
    pending: Option<RequestId>,
    next_request: u32,
    trigger: Option<ValueRef>,
    closing: bool,
    drain_after_emit: bool,
}

impl GeneratedSpeechCommitOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.closing => {
                self.trigger = Some(value);
                self.request(HostOperationId(0), value)
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && !self.closing =>
            {
                self.closing = true;
                match self.trigger {
                    Some(value) => self.request(HostOperationId(2), value),
                    None => OperationAction::Complete,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.drain_after_emit = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) if self.closing => {
                        OperationAction::Complete
                    }
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Cancelled, _, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 0,
                    }),
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(1),
                }
            }
            _ => fail(2),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.drain_after_emit {
            self.drain_after_emit = false;
            if let Some(value) = self.trigger {
                return self.request(HostOperationId(1), value);
            }
        }
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.trigger = None;
        self.closing = true;
        self.drain_after_emit = false;
    }

    fn request(&mut self, operation: HostOperationId, value: ValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        let Some(next) = self.next_request.checked_add(1) else {
            return fail(3);
        };
        let Ok(input) = BoundedValueRef::new(value, conduit_tongues::MAXIMUM_TEXT_BYTES) else {
            return fail(4);
        };
        self.next_request = next;
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        }
    }
}

pub(super) struct GeneratedSpeechCommitHost {
    committer: conduit_tongues::StreamingSpeechCommitter,
    queued: VecDeque<conduit_tongues::SpeakableSegment>,
    output: Vec<u8>,
}

impl GeneratedSpeechCommitHost {
    fn new(identity: &str) -> Result<Self, String> {
        Ok(Self {
            committer: conduit_tongues::StreamingSpeechCommitter::new(identity)
                .map_err(|error| format!("prepare speech committer: {error:?}"))?,
            queued: VecDeque::with_capacity(conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS),
            output: Vec::with_capacity(conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES as usize),
        })
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        match contract {
            conduit_std_offers::GENERATED_SPEECH_PUSH_OPERATION => {
                let delta = std::str::from_utf8(input)
                    .map_err(|_| "generated speech delta is not UTF-8")?;
                self.queued.extend(
                    self.committer
                        .push(delta)
                        .map_err(|error| format!("commit generated speech: {error:?}"))?,
                );
            }
            conduit_std_offers::GENERATED_SPEECH_DRAIN_OPERATION => {}
            conduit_std_offers::GENERATED_SPEECH_CLOSE_OPERATION => {
                self.queued.extend(
                    self.committer
                        .close()
                        .map_err(|error| format!("close generated speech: {error:?}"))?,
                );
            }
            _ => return Err("unknown generated speech commit operation".into()),
        }
        self.output.clear();
        if let Some(segment) = self.queued.pop_front() {
            self.output = serde_json::to_vec(&segment)
                .map_err(|error| format!("encode speakable segment: {error}"))?;
            Ok(Some(&self.output))
        } else {
            Ok(None)
        }
    }

    pub(super) fn cancel(&mut self) {
        self.committer.cancel();
        self.queued.clear();
        self.output.clear();
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<GeneratedSpeechCommitHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::GENERATED_SPEECH_COMMIT_IMPLEMENTATION
            {
                GeneratedSpeechCommitHost::new(placement.gear_id.as_str()).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: (conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS + 1) as u16,
        value_bytes: conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES * 2,
        host_requests: conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS * 3,
        sign_items: 64,
        maximum_value_bytes: conduit_tongues::SPEECH_COMMIT_QUEUE_BYTES,
    })
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::generated_speech_commit_offer();
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
        return Err("planned generated-speech commit differs from installed realization".into());
    }
    Ok(())
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::GeneratedSpeechCommit(
        GeneratedSpeechCommitOperation {
            pending: None,
            next_request: 0,
            trigger: None,
            closing: false,
            drain_after_emit: false,
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

    fn decode(encoded: &[u8]) -> conduit_tongues::SpeakableSegment {
        serde_json::from_slice(encoded).expect("decode segment")
    }

    #[test]
    fn stable_boundaries_flow_in_order_and_close_flushes_the_tail() {
        let mut host = GeneratedSpeechCommitHost::new("answer/test").expect("host");
        let first = host
            .execute(
                conduit_std_offers::GENERATED_SPEECH_PUSH_OPERATION,
                b"Hello there. More",
            )
            .expect("push")
            .expect("first segment");
        assert_eq!(decode(first).text, "Hello there. ");
        assert!(host
            .execute(conduit_std_offers::GENERATED_SPEECH_DRAIN_OPERATION, b"")
            .expect("drain")
            .is_none());
        let final_segment = host
            .execute(conduit_std_offers::GENERATED_SPEECH_CLOSE_OPERATION, b"")
            .expect("close")
            .expect("final segment");
        assert_eq!(decode(final_segment).text, "More");
        assert!(host
            .execute(conduit_std_offers::GENERATED_SPEECH_DRAIN_OPERATION, b"")
            .expect("final drain")
            .is_none());
    }

    #[test]
    fn cancellation_discards_pending_unsaid_text() {
        let mut host = GeneratedSpeechCommitHost::new("answer/cancel").expect("host");
        assert!(host
            .execute(
                conduit_std_offers::GENERATED_SPEECH_PUSH_OPERATION,
                b"not stable yet"
            )
            .expect("push")
            .is_none());
        host.cancel();
        assert!(host.queued.is_empty());
        assert!(host.output.is_empty());
        assert!(host
            .execute(conduit_std_offers::GENERATED_SPEECH_CLOSE_OPERATION, b"")
            .is_err());
    }
}
