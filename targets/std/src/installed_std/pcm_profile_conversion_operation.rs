//! Exact bounded PCM conversion between the initialized Piper and ALSA profiles.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) const HOST_CALL: &str = conduit_std_offers::AUDIO_CONVERT_PCM_OPERATION;
const SOURCE_RATE: u64 = 22_050;
const TARGET_RATE: u64 = 48_000;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct PcmProfileConversionOperation {
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
    closed: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for PcmProfileConversionOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            self.emitted = false;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_failure(FailureCode::InvalidLifecycle, 3);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed PCM conversion completion");
                    io.send(PortId(0), output.value)
                        .expect("ready PCM conversion output");
                    self.pending = None;
                    self.emitted = true;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed empty PCM conversion completion");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                (_, _, Some(failure)) => return StepOutcome::Fail(failure),
                _ => return step_failure(FailureCode::InvalidLifecycle, 2),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.closed {
                return step_failure(FailureCode::InvalidLifecycle, 3);
            }
            let Ok(input) = BoundedValueRef::new(value, conduit_std_offers::PIPER_PCM_BLOCK_BYTES)
            else {
                return step_failure(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_failure(FailureCode::WorkBudgetExhausted, 1);
            };
            io.consume(PortId(0)).expect("present PCM conversion input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("PCM conversion Host Call");
            self.next_request = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closed {
            io.consume_closed(PortId(0))
                .expect("observed PCM conversion closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.closed = true;
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl PcmProfileConversionOperation {}

pub(super) struct PcmProfileConversionHost {
    source_clock: Option<u64>,
    source_frames: u64,
    target_frames: u64,
}

impl PcmProfileConversionHost {
    fn new() -> Self {
        Self {
            source_clock: None,
            source_frames: 0,
            target_frames: 0,
        }
    }

    pub(super) fn convert(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Failure> {
        let (header, payload) = PcmFrameHeader::decode_frame(input).map_err(|_| Failure {
            code: FailureCode::InvalidInput,
            detail: 10,
        })?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.sample_rate_hz != SOURCE_RATE as u32
            || header.layout != PcmChannelLayout::Mono
            || header.frame_count > conduit_std_offers::PIPER_FRAMES_PER_BLOCK
            || header.discontinuity
            || header.start_frame != self.source_frames
            || self
                .source_clock
                .is_some_and(|clock| clock != header.clock_id)
        {
            return Err(Failure {
                code: FailureCode::InvalidInput,
                detail: 11,
            });
        }
        self.source_clock.get_or_insert(header.clock_id);
        let source_end = self.source_frames + u64::from(header.frame_count);
        let target_end = source_end
            .checked_mul(TARGET_RATE)
            .and_then(|value| value.checked_add(SOURCE_RATE - 1))
            .map(|value| value / SOURCE_RATE)
            .ok_or(Failure {
                code: FailureCode::WorkBudgetExhausted,
                detail: 12,
            })?;
        let frame_count = u16::try_from(target_end - self.target_frames).map_err(|_| Failure {
            code: FailureCode::WorkBudgetExhausted,
            detail: 13,
        })?;
        if frame_count == 0 || frame_count > conduit_std_offers::AUDIO_CONVERT_PCM_OUTPUT_FRAMES {
            return Err(Failure {
                code: FailureCode::WorkBudgetExhausted,
                detail: 14,
            });
        }
        let output_header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            TARGET_RATE as u32,
            PcmChannelLayout::StereoLeftRight,
            frame_count,
            crate::hosted_audio::SOURCE_CLOCK_ID,
            self.target_frames,
            false,
        )
        .map_err(|_| Failure {
            code: FailureCode::InvalidInput,
            detail: 15,
        })?;
        output.clear();
        output.extend_from_slice(&output_header.encode());
        for target_frame in self.target_frames..target_end {
            let source_frame = target_frame * SOURCE_RATE / TARGET_RATE;
            let local =
                usize::try_from(source_frame - self.source_frames).map_err(|_| Failure {
                    code: FailureCode::InvalidInput,
                    detail: 16,
                })?;
            let offset = local * 2;
            let sample = payload.get(offset..offset + 2).ok_or(Failure {
                code: FailureCode::InvalidInput,
                detail: 17,
            })?;
            output.extend_from_slice(sample);
            output.extend_from_slice(sample);
        }
        self.source_frames = source_end;
        self.target_frames = target_end;
        Ok(())
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<PcmProfileConversionHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::AUDIO_CONVERT_PCM_IMPLEMENTATION)
                .then(PcmProfileConversionHost::new)
        })
        .collect()
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_std_offers::PIPER_PCM_BLOCK_BYTES
            + conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
        host_requests: usize::from(conduit_std_offers::PIPER_MAXIMUM_BLOCKS),
        sign_items: 64,
        maximum_value_bytes: conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::PcmProfileConversion(
        PcmProfileConversionOperation {
            pending: None,
            next_request: 0,
            emitted: false,
            closed: false,
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::audio_convert_pcm_profile_offer();
    let exact_configuration = placement.configuration.iter().any(|entry| {
        entry.key.as_str() == conduit_semantic_catalog::AUDIO_CONVERT_OUTPUT_RATE_KEY
            && entry.value == ConfigurationValue::U64(TARGET_RATE)
    }) && placement.configuration.iter().any(|entry| {
        entry.key.as_str() == conduit_semantic_catalog::AUDIO_CONVERT_OUTPUT_LAYOUT_KEY
            && entry.value == ConfigurationValue::Text("stereo-left-right".into())
    });
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.inputs.len() != 1
        || placement.outputs.len() != 1
        || placement.inputs[0].direction != PortDirection::Input
        || placement.outputs[0].direction != PortDirection::Output
        || placement.configuration.len() != 2
        || !exact_configuration
    {
        return Err("planned PCM conversion does not match the exact installed profile".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(start: u64, frames: u16, clock: u64) -> Vec<u8> {
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            SOURCE_RATE as u32,
            PcmChannelLayout::Mono,
            frames,
            clock,
            start,
            false,
        )
        .unwrap();
        let payload = (0..frames)
            .flat_map(|index| {
                i16::try_from(start + u64::from(index))
                    .unwrap()
                    .to_le_bytes()
            })
            .collect::<Vec<_>>();
        header.encode_frame(&payload).unwrap()
    }

    #[test]
    fn rational_mapping_is_contiguous_across_source_blocks() {
        let mut host = PcmProfileConversionHost::new();
        let mut output =
            Vec::with_capacity(conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES as usize);
        host.convert(&block(0, 25, 7), &mut output).unwrap();
        let (first, first_payload) = PcmFrameHeader::decode_frame(&output).unwrap();
        assert_eq!((first.start_frame, first.frame_count), (0, 55));
        assert_eq!(first.sample_rate_hz, 48_000);
        assert_eq!(first.layout, PcmChannelLayout::StereoLeftRight);
        assert!(first_payload
            .as_chunks::<4>()
            .0
            .iter()
            .all(|frame| frame[..2] == frame[2..]));

        host.convert(&block(25, 25, 7), &mut output).unwrap();
        let (second, _) = PcmFrameHeader::decode_frame(&output).unwrap();
        assert_eq!((second.start_frame, second.frame_count), (55, 54));
        assert_eq!(host.source_frames, 50);
        assert_eq!(host.target_frames, 109);
    }

    #[test]
    fn malformed_wrong_profile_and_discontinuity_fail_before_state_moves() {
        let mut host = PcmProfileConversionHost::new();
        let mut output = Vec::new();
        assert_eq!(
            host.convert(&[0; 4], &mut output).unwrap_err().code,
            FailureCode::InvalidInput
        );
        let wrong_start = block(1, 4, 7);
        assert_eq!(
            host.convert(&wrong_start, &mut output).unwrap_err().detail,
            11
        );
        assert_eq!((host.source_frames, host.target_frames), (0, 0));
        host.convert(&block(0, 4, 7), &mut output).unwrap();
        assert_eq!(
            host.convert(&block(4, 4, 8), &mut output)
                .unwrap_err()
                .detail,
            11
        );
        assert_eq!(host.source_frames, 4);
    }
}
