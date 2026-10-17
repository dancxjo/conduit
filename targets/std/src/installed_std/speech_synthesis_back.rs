//! Pull-based installed operation for bounded hosted speech synthesis.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) static ESPEAK_STREAM_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static ESPEAK_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static DETERMINISTIC_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static DETERMINISTIC_STREAMING_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct SpeechSynthesisBack {
    continuation: ValueRef,
    pending: Option<RequestId>,
    next_request: u32,
    emitted_blocks: u16,
    maximum_blocks: u16,
    streaming: bool,
    input_closed: bool,
    started: bool,
    finished: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for SpeechSynthesisBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.finished {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 8);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if self.emitted_blocks < self.maximum_blocks
                        && output.admitted_bytes == conduit_std_offers::SPEECH_PCM_BLOCK_BYTES
                        && output.value.byte_len <= conduit_std_offers::SPEECH_PCM_BLOCK_BYTES =>
                {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    let input = BoundedValueRef::new(self.continuation, 1)
                        .expect("prepared speech continuation marker");
                    let next_request = RequestId(self.next_request);
                    let Some(next) = self.next_request.checked_add(1) else {
                        return step_fail(FailureCode::IdentityCapacityExhausted, 6);
                    };
                    io.consume_host_completion()
                        .expect("observed speech synthesis completion");
                    io.send(PortId(0), output.value)
                        .expect("ready speech PCM output");
                    io.request_host_call(next_request, HostCallId(0), input)
                        .expect("speech synthesis continuation Host Call");
                    self.emitted_blocks += 1;
                    self.next_request = next;
                    self.pending = Some(next_request);
                    StepOutcome::Progress
                }
                (HostCallDisposition::Completed, None, None) if self.started => {
                    io.consume_host_completion()
                        .expect("observed completed speech segment");
                    self.pending = None;
                    self.started = false;
                    if self.streaming && !self.input_closed {
                        StepOutcome::Progress
                    } else {
                        io.discard(self.continuation)
                            .expect("release speech continuation marker");
                        self.finished = true;
                        StepOutcome::Complete
                    }
                }
                (HostCallDisposition::Denied, _, Some(failure))
                | (HostCallDisposition::Cancelled, _, Some(failure))
                | (HostCallDisposition::Failed, _, Some(failure)) => StepOutcome::Fail(failure),
                (HostCallDisposition::Denied, _, None) => step_fail(FailureCode::HostCallDenied, 3),
                (HostCallDisposition::Cancelled, _, None) => step_fail(FailureCode::Cancelled, 4),
                (HostCallDisposition::Failed, _, None) => step_fail(FailureCode::HostCallFailed, 5),
                (HostCallDisposition::Completed, Some(_), None) => {
                    step_fail(FailureCode::WorkBudgetExhausted, 6)
                }
                _ => step_fail(FailureCode::InvalidLifecycle, 7),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.started || self.pending.is_some() || self.input_closed {
                return step_fail(FailureCode::InvalidLifecycle, 8);
            }
            let maximum = if self.streaming {
                conduit_tongues::MAXIMUM_ENCODED_SPEAKABLE_SEGMENT_BYTES as u32
            } else {
                conduit_tongues::MAXIMUM_TEXT_BYTES
            };
            let Ok(input) = BoundedValueRef::new(value, maximum) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            if value.byte_len == 0 {
                return step_fail(FailureCode::InvalidInput, 2);
            }
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::IdentityCapacityExhausted, 6);
            };
            io.consume(PortId(0))
                .expect("present speech synthesis input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("speech synthesis Host Call");
            self.started = true;
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if self.streaming && io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed speech synthesis stream closure");
            self.input_closed = true;
            if self.pending.is_none() && !self.started {
                io.discard(self.continuation)
                    .expect("release unused speech continuation marker");
                self.finished = true;
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            }
        } else {
            StepOutcome::Await
        }
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        value == self.continuation
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.finished = true;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl SpeechSynthesisBack {}

pub(super) fn maximum_output_bytes(placement: &PlannedGear) -> Result<u32, String> {
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-output-bytes", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "speech synthesis output bound is missing".to_string())?;
    let value = u32::try_from(value)
        .map_err(|_| "speech synthesis output bound does not fit the kernel".to_string())?;
    let maximum = if placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND {
        conduit_tongues::MAXIMUM_STREAM_PCM_BYTES
    } else {
        conduit_tongues::MAXIMUM_PCM_BYTES
    };
    if value == 0 || value > maximum {
        return Err("speech synthesis output bound is outside the portable contract".into());
    }
    Ok(value)
}

fn maximum_blocks(placement: &PlannedGear) -> Result<u16, String> {
    let bytes = maximum_output_bytes(placement)?;
    let payload_per_block = u32::from(conduit_std_offers::SPEECH_FRAMES_PER_BLOCK) * 2;
    let partial_segment_blocks =
        if placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND {
            crate::hosted_speech_synthesis::streaming::StreamLimits::from_placement(placement)
                .map_err(|error| error.to_string())?
                .maximum_segments
                - 1
        } else {
            0
        };
    let blocks = u16::try_from(bytes.div_ceil(payload_per_block) + partial_segment_blocks)
        .map_err(|_| "speech synthesis block bound does not fit the kernel".to_string())?;
    if placement.kind_id.as_str() != conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND
        && blocks > conduit_std_offers::SPEECH_MAXIMUM_BLOCKS
    {
        return Err("speech synthesis block bound exceeds its execution profile".into());
    }
    Ok(blocks)
}

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    crate::hosted_language::admit(placement)
        .map_err(|error| format!("synthesis Language preparation: {error:?}"))?;
    let offer = match placement.implementation_id.as_str() {
        conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION => {
            conduit_std_offers::deterministic_speech_offer()
        }
        conduit_std_offers::DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION => {
            conduit_std_offers::deterministic_streaming_speech_offer()
        }
        conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION
        | conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION => {
            let [resource] = placement.resources.as_slice() else {
                return Err("planned speech provider resource is missing".into());
            };
            let content = resource
                .content
                .as_ref()
                .ok_or("planned speech provider content is missing")?;
            if placement.implementation_id.as_str()
                == conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION
            {
                conduit_std_offers::espeak_streaming_offer(content.contract.clone())
            } else {
                conduit_std_offers::espeak_speech_offer(content.contract.clone())
            }
        }
        _ => return Err("planned speech implementation is not installed".into()),
    };
    if matches!(
        placement.implementation_id.as_str(),
        conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
            | conduit_std_offers::DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION
    ) && placement.realization_properties != offer.realization_properties
    {
        return Err(
            "deterministic speech proof Language coverage differs from installation".into(),
        );
    }
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.configuration.len() != offer.semantic_contract.configuration.len()
    {
        return Err("planned speech identity does not match its installation".into());
    }
    if matches!(
        placement.implementation_id.as_str(),
        conduit_std_offers::ESPEAK_SPEECH_IMPLEMENTATION
            | conduit_std_offers::ESPEAK_STREAM_IMPLEMENTATION
    ) {
        let resource = &placement.resources[0];
        if resource.class_id.as_str() != conduit_std_offers::ESPEAK_SPEECH_RESOURCE_CLASS
            || resource.units != 1
            || placement.authority.len() != 1
        {
            return Err("planned speech provider requires its exact resource and authority".into());
        }
    } else if !placement.resources.is_empty() || !placement.authority.is_empty() {
        return Err("deterministic speech proof requires no Host resource or authority".into());
    }
    maximum_output_bytes(placement)?;
    if placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND {
        crate::hosted_speech_synthesis::streaming::StreamLimits::from_placement(placement)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    let maximum_blocks = maximum_blocks(placement)?;
    let maximum_input = placement
        .host_calls
        .first()
        .ok_or("synthesis host call is missing")?
        .maximum_input_bytes;
    Ok(BackBudget {
        // The source Text remains live until its first host completion while
        // that completion stores one output block. The continuation marker is
        // retained for all later pulls.
        value_items: 3,
        value_bytes: 1 + maximum_input + conduit_std_offers::SPEECH_PCM_BLOCK_BYTES,
        host_requests: usize::from(maximum_blocks)
            + if placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND {
                crate::hosted_speech_synthesis::streaming::StreamLimits::from_placement(placement)
                    .map_err(|e| e.to_string())?
                    .maximum_segments as usize
            } else {
                1
            },
        sign_items: 64,
        maximum_value_bytes: conduit_std_offers::SPEECH_PCM_BLOCK_BYTES.max(maximum_input),
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let continuation = values
        .store(&[0])
        .map_err(|error| format!("store speech continuation marker: {error:?}"))?;
    let streaming = placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_STREAM_KIND;
    Ok(InstalledBack::SpeechSynthesis(SpeechSynthesisBack {
        continuation,
        pending: None,
        next_request: 0,
        emitted_blocks: 0,
        maximum_blocks: maximum_blocks(placement)?,
        streaming,
        input_closed: false,
        started: false,
        finished: false,
    }))
}

pub(super) struct FakeSpeechHost {
    blocks: [Vec<u8>; 3],
    next: usize,
    active: bool,
}

impl FakeSpeechHost {
    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Option<&[u8]>, String> {
        if !self.active {
            let text = core::str::from_utf8(input)
                .map_err(|_| "fake speech input is not UTF-8".to_string())?;
            if text.is_empty() {
                return Err("fake speech input is empty".into());
            }
            self.active = true;
        } else if input != [0] {
            return Err("fake speech continuation marker is malformed".into());
        }
        let output = self.blocks.get(self.next).map(Vec::as_slice);
        if output.is_some() {
            self.next += 1;
        } else {
            self.next = 0;
            self.active = false;
        }
        Ok(output)
    }
}

pub(super) fn prepare_fake_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<FakeSpeechHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if !matches!(
                placement.implementation_id.as_str(),
                conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
                    | conduit_std_offers::DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION
            ) {
                return Ok(None);
            }
            validate(placement)?;
            let blocks: [Vec<u8>; 3] = core::array::from_fn(|index| {
                let frame_count = 4;
                let header = conduit_audio::PcmFrameHeader::new(
                    conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
                    22_050,
                    conduit_audio::PcmChannelLayout::Mono,
                    frame_count,
                    0x5350_4545_4348,
                    index as u64 * u64::from(frame_count),
                    false,
                )
                .expect("fake speech profile is canonical");
                let sample = i16::try_from(index + 1).expect("three blocks fit i16");
                let mut payload = Vec::with_capacity(usize::from(frame_count) * 2);
                for _ in 0..frame_count {
                    payload.extend_from_slice(&sample.to_le_bytes());
                }
                header
                    .encode_frame(&payload)
                    .expect("fake speech block is canonical")
            });
            Ok(Some(FakeSpeechHost {
                blocks,
                next: 0,
                active: false,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests;
