//! Installed bounded language-aware commit of generated deltas to speech segments.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
use std::collections::VecDeque;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::GENERATED_SPEECH_COMMIT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct GeneratedSpeechCommitBack {
    maximum_input: u32,
    push_call: HostCallId,
    drain_call: HostCallId,
    close_call: HostCallId,
    pending: Option<RequestId>,
    next_request: u32,
    trigger: Option<ValueRef>,
    closing: bool,
    drain_after_emit: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for GeneratedSpeechCommitBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 2);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let Some(trigger) = self.trigger else {
                        return step_fail(FailureCode::InvalidLifecycle, 1);
                    };
                    let Some((next_request, next)) = self.next_request() else {
                        return step_fail(FailureCode::StorageExhausted, 3);
                    };
                    let input = match BoundedValueRef::new(trigger, self.maximum_input) {
                        Ok(input) => input,
                        Err(_) => return step_fail(FailureCode::InvalidInput, 4),
                    };
                    io.consume_host_completion()
                        .expect("observed generated-speech completion");
                    io.send(PortId(0), output.value)
                        .expect("ready generated speech segment output");
                    io.request_host_call(next_request, self.drain_call, input)
                        .expect("generated-speech drain Host Call");
                    self.next_request = next;
                    self.pending = Some(next_request);
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed empty generated-speech completion");
                    self.pending = None;
                    if self.closing {
                        // Keep the retention identity through this staged completion:
                        // the explicit discard owns its single release.
                        if let Some(trigger) = self.trigger {
                            io.discard(trigger)
                                .expect("finished generated-speech trigger");
                        }
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return step_fail(FailureCode::Cancelled, 0)
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return step_fail(FailureCode::InvalidLifecycle, 1),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.closing {
                return step_fail(FailureCode::InvalidLifecycle, 2);
            }
            let input = match BoundedValueRef::new(value, self.maximum_input) {
                Ok(input) => input,
                Err(_) => return step_fail(FailureCode::InvalidInput, 4),
            };
            let Some((request, next)) = self.next_request() else {
                return step_fail(FailureCode::StorageExhausted, 3);
            };
            if self.trigger == Some(value) {
                io.consume(PortId(0))
                    .expect("repeated generated text delta identity");
            } else {
                if let Some(previous) = self.trigger.take() {
                    io.discard(previous)
                        .expect("superseded generated-speech trigger");
                }
                self.trigger = Some(
                    io.take_input(PortId(0))
                        .expect("present generated text delta"),
                );
            }
            io.request_host_call(request, self.push_call, input)
                .expect("generated-speech push Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closing {
            io.consume_closed(PortId(0))
                .expect("observed generated text closure");
            self.closing = true;
            let Some(trigger) = self.trigger else {
                return StepOutcome::Complete;
            };
            let input = match BoundedValueRef::new(trigger, self.maximum_input) {
                Ok(input) => input,
                Err(_) => return step_fail(FailureCode::InvalidInput, 4),
            };
            let Some((request, next)) = self.next_request() else {
                return step_fail(FailureCode::StorageExhausted, 3);
            };
            io.request_host_call(request, self.close_call, input)
                .expect("generated-speech close Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        self.trigger == Some(value)
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.trigger = None;
        self.closing = true;
        self.drain_after_emit = false;
    }
}

impl GeneratedSpeechCommitBack {
    fn next_request(&self) -> Option<(RequestId, u32)> {
        self.next_request
            .checked_add(1)
            .map(|next| (RequestId(self.next_request), next))
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl GeneratedSpeechCommitBack {}

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
            serde_json::to_writer(&mut self.output, &segment)
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

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
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
        || placement.host_calls != offer.host_calls
        || placement.semantic_contract != offer.semantic_contract
        || placement.limits != offer.limits
        || !placement.configuration.is_empty()
    {
        return Err("planned generated-speech commit differs from installed realization".into());
    }
    Ok(())
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::GeneratedSpeechCommit(
        GeneratedSpeechCommitBack {
            maximum_input: placement.host_calls[0].maximum_input_bytes,
            push_call: operation_index(
                placement,
                conduit_std_offers::GENERATED_SPEECH_PUSH_OPERATION,
            )?,
            drain_call: operation_index(
                placement,
                conduit_std_offers::GENERATED_SPEECH_DRAIN_OPERATION,
            )?,
            close_call: operation_index(
                placement,
                conduit_std_offers::GENERATED_SPEECH_CLOSE_OPERATION,
            )?,
            pending: None,
            next_request: 0,
            trigger: None,
            closing: false,
            drain_after_emit: false,
        },
    ))
}

fn operation_index(placement: &PlannedGear, operation: &str) -> Result<HostCallId, String> {
    let index = placement
        .host_calls
        .iter()
        .position(|call| call.contract_id.as_str() == operation)
        .ok_or_else(|| format!("missing generated-speech operation {operation}"))?;
    Ok(HostCallId(index as u16))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(encoded: &[u8]) -> conduit_tongues::SpeakableSegment {
        serde_json::from_slice(encoded).expect("decode segment")
    }

    #[test]
    fn closing_completion_retains_input_until_its_explicit_discard_commits() {
        let value = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 5,
        };
        let mut back = GeneratedSpeechCommitBack {
            maximum_input: 1024,
            push_call: HostCallId(2),
            drain_call: HostCallId(1),
            close_call: HostCallId(0),
            pending: Some(RequestId(3)),
            next_request: 4,
            trigger: Some(value),
            closing: true,
            drain_after_emit: false,
        };
        let mut io = StepIo::<1>::test_frame(
            [None],
            [false],
            [None],
            Some((
                RequestId(3),
                conduit_kernel::HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
            8,
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(StepBack::<1>::retains_host_call_input(
            &back,
            RequestId(3),
            value
        ));
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
