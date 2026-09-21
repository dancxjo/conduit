//! Installed std realization of explicit Tongues single-shot/streaming adapters.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_audio::{
    PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, MAXIMUM_PCM_CLIP_FRAMES,
    MAXIMUM_PCM_FRAMES_PER_BLOCK,
};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

const TARGET_RATE_HZ: u32 = 16_000;

pub(super) static WINDOW_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::SPEECH_WINDOW_TO_CLIP_STD_IMPLEMENTATION,
    budget: window_budget,
    prepare: prepare_window,
};

pub(super) static RESULT_STREAM_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::SPEECH_RESULT_TO_EVENT_STREAM_STD_IMPLEMENTATION,
    budget: result_budget,
    prepare: prepare_result_stream,
};

pub(super) struct SpeechWindowToClipOperation {
    pending: Option<RequestId>,
    next_request: u32,
    trigger: ValueRef,
    closing: bool,
    emitted: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for SpeechWindowToClipOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            if self.closing {
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        if !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        io.consume_host_completion()
                            .expect("observed speech window close completion");
                        io.send(PortId(0), output.value)
                            .expect("ready speech clip output");
                        self.pending = None;
                        self.emitted = true;
                        StepOutcome::Progress
                    }
                    (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 0),
                    (_, _, Some(failure)) => StepOutcome::Fail(failure),
                    _ => step_fail(FailureCode::InvalidLifecycle, 5),
                }
            } else {
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, None, None) => {
                        io.consume_host_completion()
                            .expect("observed speech window append completion");
                        self.pending = None;
                        StepOutcome::Progress
                    }
                    (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 0),
                    (_, _, Some(failure)) => StepOutcome::Fail(failure),
                    _ => step_fail(FailureCode::InvalidLifecycle, 3),
                }
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.closing {
                return step_fail(FailureCode::InvalidLifecycle, 6);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                    + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
            ) else {
                return step_fail(FailureCode::InvalidInput, 1);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return step_fail(FailureCode::StorageExhausted, 2);
            };
            io.consume(PortId(0)).expect("present speech PCM frame");
            io.request_host_call(request, HostCallId(0), input)
                .expect("speech window append Host Call");
            self.next_request = next;
            self.pending = Some(request);
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) && self.pending.is_none() && !self.closing {
            let request = RequestId(self.next_request);
            let input = BoundedValueRef::new(self.trigger, 1)
                .expect("speech window close trigger is one byte");
            io.consume_closed(PortId(0))
                .expect("observed speech PCM closure");
            io.request_host_call(request, HostCallId(1), input)
                .expect("speech window close Host Call");
            self.closing = true;
            self.pending = Some(request);
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.closing = true;
    }
}

impl SpeechWindowToClipOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.closing && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_audio::MAXIMUM_PCM_FRAME_BYTES
                        + conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN as u32,
                ) else {
                    return fail(1);
                };
                let request = RequestId(self.next_request);
                let Some(next) = self.next_request.checked_add(1) else {
                    return fail(2);
                };
                self.next_request = next;
                self.pending = Some(request);
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) && !self.closing =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, None, None) => OperationAction::Await,
                    (HostCallDisposition::Cancelled, _, _) => cancelled(),
                    (_, _, Some(failure)) => OperationAction::Fail(failure),
                    _ => fail(3),
                }
            }
            OperationInput::Closed { port: PortId(0) }
                if self.pending.is_none() && !self.closing && !self.emitted =>
            {
                self.closing = true;
                let request = RequestId(self.next_request);
                let Ok(input) = BoundedValueRef::new(self.trigger, 1) else {
                    return fail(4);
                };
                self.pending = Some(request);
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(1),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) && self.closing =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Cancelled, _, _) => cancelled(),
                    (_, _, Some(failure)) => OperationAction::Fail(failure),
                    _ => fail(5),
                }
            }
            _ => fail(6),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.closing = true;
    }
}

pub(super) struct SpeechResultToEventStreamOperation {
    pending: Option<RequestId>,
    emitted: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for SpeechResultToEventStreamOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 22);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed speech result adaptation completion");
                    io.send(PortId(0), output.value)
                        .expect("ready recognition event output");
                    self.pending = None;
                    self.emitted = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, _, _) => step_fail(FailureCode::Cancelled, 0),
                (_, _, Some(failure)) => StepOutcome::Fail(failure),
                _ => step_fail(FailureCode::InvalidLifecycle, 21),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return step_fail(FailureCode::InvalidLifecycle, 22);
            }
            let Ok(input) = BoundedValueRef::new(
                value,
                conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
            ) else {
                return step_fail(FailureCode::InvalidInput, 20);
            };
            io.consume(PortId(0))
                .expect("present speech recognition result");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("speech result adaptation Host Call");
            self.pending = Some(RequestId(0));
            StepOutcome::Progress
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

impl SpeechResultToEventStreamOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES as u32,
                ) else {
                    return fail(20);
                };
                self.pending = Some(RequestId(0));
                OperationAction::RequestHostCall {
                    request: RequestId(0),
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Cancelled, _, _) => cancelled(),
                    (_, _, Some(failure)) => OperationAction::Fail(failure),
                    _ => fail(21),
                }
            }
            _ => fail(22),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) struct SpeechWindowToClipHost {
    source_rate_hz: Option<u32>,
    source_layout: Option<PcmChannelLayout>,
    source_clock_id: Option<u64>,
    expected_start_frame: Option<u64>,
    source_frames: u64,
    target_frames: u64,
    window: conduit_tongues::AcousticWindow,
}

impl SpeechWindowToClipHost {
    fn new() -> Self {
        Self {
            source_rate_hz: None,
            source_layout: None,
            source_clock_id: None,
            expected_start_frame: None,
            source_frames: 0,
            target_frames: 0,
            window: conduit_tongues::AcousticWindow::new(MAXIMUM_PCM_CLIP_FRAMES as usize * 2)
                .expect("canonical Whisper turn fits the portable AcousticWindow bound"),
        }
    }

    pub(super) fn push(&mut self, input: &[u8]) -> Result<(), String> {
        let (header, payload) = PcmFrameHeader::decode_frame(input)
            .map_err(|error| format!("speech window PCM frame: {error:?}"))?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.discontinuity
        {
            return Err("speech window requires contiguous signed-16 PCM".into());
        }
        if let Some(rate) = self.source_rate_hz {
            if rate != header.sample_rate_hz
                || self.source_layout != Some(header.layout)
                || self.source_clock_id != Some(header.clock_id)
                || self.expected_start_frame != Some(header.start_frame)
            {
                return Err("speech window PCM profile or clock changed mid-turn".into());
            }
        } else {
            self.source_rate_hz = Some(header.sample_rate_hz);
            self.source_layout = Some(header.layout);
            self.source_clock_id = Some(header.clock_id);
            self.expected_start_frame = Some(header.start_frame);
        }

        let source_rate = u64::from(header.sample_rate_hz);
        let source_begin = self.source_frames;
        let source_end = source_begin
            .checked_add(u64::from(header.frame_count))
            .ok_or_else(|| "speech window source extent overflow".to_string())?;
        let target_end = source_end
            .checked_mul(u64::from(TARGET_RATE_HZ))
            .and_then(|value| value.checked_add(source_rate - 1))
            .map(|value| value / source_rate)
            .ok_or_else(|| "speech window target extent overflow".to_string())?;
        if target_end > u64::from(MAXIMUM_PCM_CLIP_FRAMES) {
            return Err("speech recognition window exceeded its finite turn extent".into());
        }

        let channels = usize::from(header.layout.channels());
        let target_items = usize::try_from(target_end - self.target_frames)
            .map_err(|_| "speech window target item count overflow".to_string())?;
        let mut normalized_block = Vec::with_capacity(target_items.saturating_mul(2));
        for target in self.target_frames..target_end {
            let source = target
                .checked_mul(source_rate)
                .map(|value| value / u64::from(TARGET_RATE_HZ))
                .ok_or_else(|| "speech window resampling extent overflow".to_string())?;
            if source < source_begin || source >= source_end {
                return Err("speech window resampling crossed an unretained source block".into());
            }
            let local = usize::try_from(source - source_begin)
                .map_err(|_| "speech window source index overflow".to_string())?;
            let sample = match header.layout {
                PcmChannelLayout::Mono => read_sample(payload, local)?,
                PcmChannelLayout::StereoLeftRight => {
                    let left = i32::from(read_sample(payload, local * channels)?);
                    let right = i32::from(read_sample(payload, local * channels + 1)?);
                    ((left + right) / 2) as i16
                }
            };
            normalized_block.extend_from_slice(&sample.to_le_bytes());
        }
        self.window
            .push(&normalized_block)
            .map_err(|error| format!("speech AcousticWindow refused normalized PCM: {error:?}"))?;

        self.source_frames = source_end;
        self.target_frames = target_end;
        self.expected_start_frame = header
            .start_frame
            .checked_add(u64::from(header.frame_count));
        Ok(())
    }

    pub(super) fn close(&mut self) -> Result<Vec<u8>, String> {
        if self.window.retained_bytes() == 0 {
            return Err("speech recognition window closed without audio".into());
        }
        let clock = self
            .source_clock_id
            .ok_or_else(|| "speech recognition window has no source clock".to_string())?;
        let raw = self.window.window();
        if !raw.len().is_multiple_of(2) {
            return Err("speech AcousticWindow retained a partial signed-16 sample".into());
        }
        let maximum_payload_bytes = usize::from(MAXIMUM_PCM_FRAMES_PER_BLOCK) * 2;
        let mut encoded_frames = Vec::with_capacity(raw.len().div_ceil(maximum_payload_bytes));
        let mut start = 0_u64;
        for chunk in raw.chunks(maximum_payload_bytes) {
            let frame_count = u16::try_from(chunk.len() / 2)
                .map_err(|_| "speech window frame count overflow".to_string())?;
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                TARGET_RATE_HZ,
                PcmChannelLayout::Mono,
                frame_count,
                clock,
                start,
                false,
            )
            .map_err(|error| format!("speech window normalized header: {error:?}"))?;
            encoded_frames.push(
                header
                    .encode_frame(chunk)
                    .map_err(|error| format!("speech window normalized frame: {error:?}"))?,
            );
            start += u64::from(frame_count);
        }
        let borrowed = encoded_frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let output = conduit_audio::encode_pcm_clip(&borrowed)
            .map_err(|error| format!("encode speech recognition clip: {error:?}"))?;
        self.window.release();
        Ok(output)
    }

    pub(super) fn cancel(&mut self) {
        self.window.cancel();
    }
}

pub(super) struct SpeechResultToEventStreamHost;

impl SpeechResultToEventStreamHost {
    fn new() -> Self {
        Self
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        conduit_tongues::recognition_result_to_terminal_event(input)
            .map_err(|error| format!("adapt recognition result to event stream: {error:?}"))
    }

    pub(super) fn cancel(&mut self) {}
}

pub(super) fn prepare_window_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<SpeechWindowToClipHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::SPEECH_WINDOW_TO_CLIP_STD_IMPLEMENTATION)
                .then(SpeechWindowToClipHost::new)
        })
        .collect()
}

pub(super) fn prepare_result_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<SpeechResultToEventStreamHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::SPEECH_RESULT_TO_EVENT_STREAM_STD_IMPLEMENTATION)
                .then(SpeechResultToEventStreamHost::new)
        })
        .collect()
}

fn read_sample(payload: &[u8], sample_index: usize) -> Result<i16, String> {
    let offset = sample_index
        .checked_mul(2)
        .ok_or_else(|| "speech sample offset overflow".to_string())?;
    let bytes = payload
        .get(offset..offset + 2)
        .ok_or_else(|| "speech PCM sample is outside its frame".to_string())?;
    Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn window_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_window(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32 + 1,
        host_requests: conduit_tongues::MAXIMUM_ACOUSTIC_WINDOW_ITEMS + 1,
        sign_items: 64,
        maximum_value_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
    })
}

fn result_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_result(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES
            + conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES) as u32,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_tongues::MAXIMUM_RECOGNITION_EVENT_BYTES as u32,
    })
}

fn prepare_window(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_window(placement)?;
    let trigger = values
        .store(&[0])
        .map_err(|error| format!("store speech window close trigger: {error:?}"))?;
    Ok(InstalledOperation::SpeechWindowToClip(
        SpeechWindowToClipOperation {
            pending: None,
            next_request: 0,
            trigger,
            closing: false,
            emitted: false,
        },
    ))
}

fn prepare_result_stream(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_result(placement)?;
    Ok(InstalledOperation::SpeechResultToEventStream(
        SpeechResultToEventStreamOperation {
            pending: None,
            emitted: false,
        },
    ))
}

fn validate_window(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::speech_window_to_clip_std_offer(),
    )
}

fn validate_result(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        &conduit_std_offers::speech_result_to_event_stream_std_offer(),
    )
}

fn validate(placement: &PlannedGear, offer: &conduit_core::CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned speech recognition adapter differs from installed realization".into());
    }
    Ok(())
}

fn cancelled() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::Cancelled,
        detail: 0,
    })
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

    fn frame(rate: u32, layout: PcmChannelLayout, start: u64, frames: u16) -> Vec<u8> {
        let channels = usize::from(layout.channels());
        let mut payload = Vec::with_capacity(usize::from(frames) * channels * 2);
        for index in 0..usize::from(frames) {
            let value = (index as i16).wrapping_mul(8);
            for _ in 0..channels {
                payload.extend_from_slice(&value.to_le_bytes());
            }
        }
        PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            rate,
            layout,
            frames,
            77,
            start,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap()
    }

    #[test]
    fn browser_shaped_pcm_normalizes_to_one_canonical_whisper_clip() {
        let mut host = SpeechWindowToClipHost::new();
        host.push(&frame(48_000, PcmChannelLayout::StereoLeftRight, 0, 480))
            .unwrap();
        host.push(&frame(48_000, PcmChannelLayout::StereoLeftRight, 480, 480))
            .unwrap();
        let encoded = host.close().unwrap();
        let clip = conduit_audio::decode_pcm_clip(&encoded).unwrap();
        assert_eq!(clip.profile.sample_rate_hz, 16_000);
        assert_eq!(clip.profile.layout, PcmChannelLayout::Mono);
        assert_eq!(clip.frame_count, 320);
    }
}
